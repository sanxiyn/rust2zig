static mut DROP_LOG: [i32; 16] = [0; 16];
static mut DROP_LEN: usize = 0;

fn log_drop(id: i32) {
    unsafe {
        let i = DROP_LEN;
        if i < 16 {
            DROP_LOG[i] = id;
            DROP_LEN = i + 1;
        }
    }
}

#[cfg(test)]
fn reset_drops() {
    unsafe {
        DROP_LEN = 0;
    }
}

#[cfg(test)]
fn n_drops() -> usize {
    unsafe { DROP_LEN }
}

#[cfg(test)]
fn drop_id(i: usize) -> i32 {
    unsafe { DROP_LOG[i] }
}

pub struct Ticket {
    pub id: i32,
}

impl Drop for Ticket {
    fn drop(&mut self) {
        log_drop(self.id);
    }
}

impl Ticket {
    pub fn new(id: i32) -> Ticket {
        Ticket { id }
    }
}

pub fn scope_end() -> i32 {
    let t = Ticket::new(1);
    t.id
}

pub fn early_return(early: bool) -> i32 {
    let t = Ticket::new(2);
    if early {
        return t.id;
    }
    t.id
}

pub fn move_out() -> Ticket {
    let t = Ticket::new(3);
    t
}

pub fn consume(t: Ticket) -> i32 {
    t.id
}

pub fn forward(t: Ticket) -> Ticket {
    t
}

pub fn nested() -> i32 {
    let outer = Ticket::new(4);
    let n = {
        let inner = Ticket::new(5);
        inner.id
    };
    outer.id + n
}

pub fn drop_order() {
    let a = Ticket::new(6);
    let b = Ticket::new(7);
    let _ = a.id + b.id;
}

#[test]
fn test_scope_end() {
    reset_drops();
    assert_eq!(1, scope_end());
    assert_eq!(1, n_drops());
    assert_eq!(1, drop_id(0));
}

#[test]
fn test_early_return_true() {
    reset_drops();
    assert_eq!(2, early_return(true));
    assert_eq!(1, n_drops());
    assert_eq!(2, drop_id(0));
}

#[test]
fn test_early_return_false() {
    reset_drops();
    assert_eq!(2, early_return(false));
    assert_eq!(1, n_drops());
    assert_eq!(2, drop_id(0));
}

#[test]
fn test_move_out() {
    reset_drops();
    let t = move_out();
    assert_eq!(3, t.id);
    assert_eq!(0, n_drops());
    drop(t);
    assert_eq!(1, n_drops());
    assert_eq!(3, drop_id(0));
}

#[test]
fn test_consume() {
    reset_drops();
    assert_eq!(8, consume(Ticket::new(8)));
    assert_eq!(1, n_drops());
    assert_eq!(8, drop_id(0));
}

#[test]
fn test_forward() {
    reset_drops();
    let t = forward(Ticket::new(9));
    assert_eq!(0, n_drops());
    assert_eq!(9, t.id);
    drop(t);
    assert_eq!(1, n_drops());
    assert_eq!(9, drop_id(0));
}

#[test]
fn test_nested() {
    reset_drops();
    assert_eq!(9, nested());
    assert_eq!(2, n_drops());
    assert_eq!(5, drop_id(0));
    assert_eq!(4, drop_id(1));
}

#[test]
fn test_drop_order() {
    reset_drops();
    drop_order();
    assert_eq!(2, n_drops());
    assert_eq!(7, drop_id(0));
    assert_eq!(6, drop_id(1));
}
