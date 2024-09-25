use crate::FaceInImage;

/// Interface for manipulating and displaying a cropped and labelled face in the UI.
pub trait Face {
    /// UI backend-specific information needed to dislpay the face
    fn as_bytes(&self, face: &FaceInImage) -> Vec<u8>;
}
