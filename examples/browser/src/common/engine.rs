use std::sync::Arc;

use gosub_engine::cookies::{CookieStoreHandle, SqliteCookieStore};
use gosub_engine::events::EngineEvent;
use gosub_engine::storage::{InMemorySessionStore, PartitionPolicy, SqliteLocalStore, StorageService};
use gosub_engine::tab::{TabDefaults, TabHandle, TabId};
use gosub_engine::zone::{Zone, ZoneConfig, ZoneId, ZoneServices};
use gosub_engine::GosubEngine;
use gosub_render_pipeline::render::backend::RenderBackend;
use gosub_render_pipeline::render::{DefaultCompositor, Viewport};
use parking_lot::RwLock;
use tokio::sync::broadcast;

/// Per-binary knobs for [`setup_engine`].
pub struct SetupOptions {
    /// Stable zone id so storage/cookies survive restarts of the same binary.
    pub zone_uuid: uuid::Uuid,
    pub tab_title: &'static str,
    /// Sqlite path for localStorage; `":memory:"` for ephemeral.
    pub local_store_path: &'static str,
    /// `None` lets the toolkit's first resize set the viewport with the correct DPR.
    pub initial_viewport: Option<Viewport>,
}

/// Everything a shell needs after engine startup.
pub struct EngineSetup {
    pub engine: GosubEngine,
    pub zone: Zone,
    pub tab: TabHandle,
    pub tab_id: TabId,
    pub event_rx: broadcast::Receiver<EngineEvent>,
}

/// Builds and starts the engine, creates the zone (with the shared sqlite cookie
/// store) and a single tab.
///
/// Callers must already be inside the shared runtime (`rt::rt().enter()` guard or
/// a runtime thread): the engine spawns its workers onto the ambient runtime.
pub fn setup_engine(
    backend: Arc<dyn RenderBackend + Send + Sync>,
    compositor: Arc<RwLock<DefaultCompositor>>,
    opts: &SetupOptions,
) -> EngineSetup {
    let mut engine = GosubEngine::new(None, backend, compositor);
    let _join = engine.start().expect("engine start");
    let event_rx = engine.subscribe_events();

    let zone_cfg = ZoneConfig::builder().do_not_track(true).build().expect("ZoneConfig");

    let cookie_store: CookieStoreHandle = SqliteCookieStore::new(".pipeline-browser-cookies.db".into())
        .expect("failed to open cookie store")
        .into();

    let zone_services = ZoneServices {
        storage: Arc::new(StorageService::new(
            Arc::new(SqliteLocalStore::new(opts.local_store_path).expect("local store")),
            Arc::new(InMemorySessionStore::new()),
        )),
        cookie_store: Some(cookie_store),
        cookie_jar: None,
        partition_policy: PartitionPolicy::None,
    };

    let mut zone = engine
        .create_zone(zone_cfg, zone_services, Some(ZoneId::from(opts.zone_uuid)))
        .expect("create_zone");

    let tab = crate::common::rt::rt()
        .block_on(zone.create_tab(
            TabDefaults {
                url: None,
                title: Some(opts.tab_title.to_string()),
                viewport: opts.initial_viewport,
            },
            None,
        ))
        .expect("create_tab");

    let tab_id = tab.tab_id;

    EngineSetup {
        engine,
        zone,
        tab,
        tab_id,
        event_rx,
    }
}
