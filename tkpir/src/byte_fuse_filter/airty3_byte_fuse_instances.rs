use super::abs_arity3_byte_fuse_instances::AbsArity3ByteFuseInstance;

pub struct Arity3ByteFuseInstance {
    pub abs_arity3_byte_fuse_ins: AbsArity3ByteFuseInstance,
}

impl Arity3ByteFuseInstance {
    pub fn new(size: usize, value_byte_len: usize) -> Self {
        Self {
            abs_arity3_byte_fuse_ins: AbsArity3ByteFuseInstance::new(size, value_byte_len),
        }
    }
}
