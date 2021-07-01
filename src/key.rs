pub(crate) struct Key<const N: usize>(Box<[u8]>);

impl<const N: usize> Key<{ N }> {
    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
    pub(crate) fn len(&self) -> u64 {
        self.0.len() as u64
    }
}

impl From<u128> for Key<{ 128 }> {
    fn from(k: u128) -> Self {
        Self(k.to_le_bytes().to_vec().into_boxed_slice())
    }
}

impl From<u64> for Key<{ 64 }> {
    fn from(k: u64) -> Self {
        Self(k.to_le_bytes().to_vec().into_boxed_slice())
    }
}

impl From<u32> for Key<32> {
    fn from(k: u32) -> Self {
        Self(k.to_le_bytes().to_vec().into_boxed_slice())
    }
}

impl<const N: usize> From<[u8; N]> for Key<{ N }> {
    fn from(k: [u8; N]) -> Self {
        Self(k.to_vec().into_boxed_slice())
    }
}
