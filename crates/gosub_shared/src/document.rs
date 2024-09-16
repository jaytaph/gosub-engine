use std::cell::{Ref, RefCell, RefMut};
use std::fmt::Debug;
use std::rc::Rc;
use crate::traits::document::Document;

pub struct DocumentHandle<D: Document>(pub Rc<RefCell<D>>);


impl<D: Document> DocumentHandle<D> {
    /// Create a new DocumentHandle from a document
    pub fn create(document: D) -> Self {
        DocumentHandle(Rc::new(RefCell::new(document)))
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

impl<D: Document + Debug> Debug for DocumentHandle<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0.borrow())
    }
}

// impl<D: Document + PartialEq> PartialEq for DocumentHandle<D> {
//     fn eq(&self, other: &Self) -> bool {
//         self.0.eq(&other.0)
//     }
// }

impl<D: Document + PartialEq> PartialEq for DocumentHandle<D> {
    fn eq(&self, other: &Self) -> bool {
        self.0.borrow().eq(&other.0.borrow())
    }
}


impl<D: Document + Eq> Eq for DocumentHandle<D> {}

// NOTE: it is preferred to use Document::clone() when
// copying a DocumentHandle reference. However, for
// any structs using this handle that use #[derive(Clone)],
// this implementation is required.
impl<D: Document> Clone for DocumentHandle<D>  {
    fn clone(&self) -> DocumentHandle<D> {
        DocumentHandle(Rc::clone(&self.0))
    }
}
