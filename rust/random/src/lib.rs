pub struct Rand32 {
    state: u64,
    inc: u64,
}

impl Rand32 {
    pub const DEFAULT_INC: u64 = 1442695040888963407;
    pub const MULTIPLIER: u64 = 6364136223846793005;

    pub fn new(seed: u64) -> Self {
        Self::new_inc(seed, Self::DEFAULT_INC)
    }

    pub fn new_inc(seed: u64, increment: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: increment.wrapping_shl(1) | 1,
        };
        let _ = rng.rand_u32();
        rng.state = rng.state.wrapping_add(seed);
        let _ = rng.rand_u32();
        rng
    }

    pub fn rand_u32(&mut self) -> u32 {
        let oldstate: u64 = self.state;
        self.state = oldstate
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(self.inc);
        let xorshifted: u32 = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot: u32 = (oldstate >> 59) as u32;
        xorshifted.rotate_right(rot)
    }
}

#[test]
fn test_rand32() {
    let seed = 54321;
    let mut r1 = Rand32::new(seed);
    assert_eq!(2891073575, r1.rand_u32());
}
