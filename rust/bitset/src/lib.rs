pub struct BitSet {
    data: usize,
    length: usize,
}

impl BitSet {
    pub fn with_capacity(bits: usize) -> Self {
        BitSet {
            data: 0,
            length: bits,
        }
    }

    pub fn contains(&self, bit: usize) -> bool {
        bit < self.length && self.data & (1 << bit) != 0
    }

    pub fn put(&mut self, bit: usize) -> bool {
        assert!(bit < self.length);
        let prev = self.data & (1 << bit) != 0;
        self.data |= 1 << bit;
        prev
    }

    pub fn toggle(&mut self, bit: usize) {
        assert!(bit < self.length);
        self.data ^= 1 << bit;
    }
}

#[test]
fn test_toggle() {
    let mut b = BitSet::with_capacity(16);
    b.toggle(1);
    b.put(2);
    b.toggle(2);
    b.put(3);
    assert!(b.contains(1));
    assert!(!b.contains(2));
    assert!(b.contains(3));
}
