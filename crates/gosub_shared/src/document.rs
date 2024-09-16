use std::cell::{Ref, RefCell, RefMut};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::rc::Rc;
use crate::traits::css3::CssSystem;
use crate::traits::document::Document;

pub struct DocumentHandle<D: Document<C>, C: CssSystem>(pub Rc<RefCell<D>>, pub PhantomData<C>);


impl<C, D> DocumentHandle<D, C>
where C: CssSystem, D: Document<C> {
    /// Create a new DocumentHandle from a document
    pub fn create(document: D) -> Self {
        DocumentHandle(Rc::new(RefCell::new(document)), PhantomData)
    }
    
    /// Returns the document as referenced by the handle
    pub fn get(&self) -> Ref<D> {
        self.0.borrow()
    }

    /// Returns a 
    pub fn get_mut(&mut self) -> RefMut<D> {
        self.0.borrow_mut()
    }
}

impl<S: CssSystem, D: Document<S> + Debug> Debug for DocumentHandle<D, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.borrow())
    }
}

// impl<D: Document + PartialEq> PartialEq for DocumentHandle<D> {
//     fn eq(&self, other: &Self) -> bool {
//         self.0.eq(&other.0)
//     }
// }

impl<S: CssSystem, D: Document<S> + PartialEq> PartialEq for DocumentHandle<D, S> {
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().eq(&other.0.borrow())
    }
}


impl<S: CssSystem, D: Document<S> + Eq> Eq for DocumentHandle<D, S> {}

// NOTE: it is preferred to use Document::clone() when
// copying a DocumentHandle reference. However, for
// any structs using this handle that use #[derive(Clone)],
// this implementation is required.
impl<S: CssSystem, D: Document<S>> Clone for DocumentHandle<D, S>  {
    fn clone(&self) -> DocumentHandle<D, S> {
        DocumentHandle(Rc::clone(&self.0), PhantomData)
    }
}
