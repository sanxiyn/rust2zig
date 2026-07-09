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

pub fn consume(t: Ticket) -> i32 {
    t.id
}

pub fn maybe_consume(c: bool) -> i32 {
    let t = Ticket::new(1);
    if c {
        return consume(t);
    }
    t.id
}

pub fn if_else(c: bool) -> i32 {
    let t = Ticket::new(2);
    if c {
        consume(t)
    } else {
        t.id
    }
}

pub fn early_move(c: bool) -> Ticket {
    let t = Ticket::new(3);
    if c {
        return t;
    }
    Ticket::new(30)
}

pub fn match_move(c: bool) -> i32 {
    let t = Ticket::new(4);
    match c {
        true => consume(t),
        false => t.id,
    }
}

pub fn nested_if(a: bool, b: bool) -> i32 {
    let t = Ticket::new(5);
    if a {
        if b {
            return consume(t);
        }
        return t.id;
    }
    t.id
}

pub fn two_tickets(c: bool) -> i32 {
    let a = Ticket::new(6);
    let b = Ticket::new(7);
    if c {
        let _ = consume(a);
        b.id
    } else {
        let _ = consume(b);
        a.id
    }
}

#[test]
fn test_maybe_consume_true() {
    reset_drops();
    assert_eq!(1, maybe_consume(true));
    assert_eq!(1, n_drops());
    assert_eq!(1, drop_id(0));
}

#[test]
fn test_maybe_consume_false() {
    reset_drops();
    assert_eq!(1, maybe_consume(false));
    assert_eq!(1, n_drops());
    assert_eq!(1, drop_id(0));
}

#[test]
fn test_if_else_true() {
    reset_drops();
    assert_eq!(2, if_else(true));
    assert_eq!(1, n_drops());
    assert_eq!(2, drop_id(0));
}

#[test]
fn test_if_else_false() {
    reset_drops();
    assert_eq!(2, if_else(false));
    assert_eq!(1, n_drops());
    assert_eq!(2, drop_id(0));
}

#[test]
fn test_early_move_true() {
    reset_drops();
    let t = early_move(true);
    assert_eq!(3, t.id);
    assert_eq!(0, n_drops());
    drop(t);
    assert_eq!(1, n_drops());
    assert_eq!(3, drop_id(0));
}

#[test]
fn test_early_move_false() {
    reset_drops();
    let t = early_move(false);
    assert_eq!(30, t.id);
    assert_eq!(1, n_drops());
    assert_eq!(3, drop_id(0));
    drop(t);
    assert_eq!(2, n_drops());
    assert_eq!(30, drop_id(1));
}

#[test]
fn test_match_move_true() {
    reset_drops();
    assert_eq!(4, match_move(true));
    assert_eq!(1, n_drops());
    assert_eq!(4, drop_id(0));
}

#[test]
fn test_match_move_false() {
    reset_drops();
    assert_eq!(4, match_move(false));
    assert_eq!(1, n_drops());
    assert_eq!(4, drop_id(0));
}

#[test]
fn test_nested_if() {
    reset_drops();
    assert_eq!(5, nested_if(true, true));
    assert_eq!(1, n_drops());
    assert_eq!(5, drop_id(0));

    reset_drops();
    assert_eq!(5, nested_if(true, false));
    assert_eq!(1, n_drops());
    assert_eq!(5, drop_id(0));

    reset_drops();
    assert_eq!(5, nested_if(false, true));
    assert_eq!(1, n_drops());
    assert_eq!(5, drop_id(0));
}

#[test]
fn test_two_tickets_true() {
    reset_drops();
    assert_eq!(7, two_tickets(true));
    assert_eq!(2, n_drops());
    assert_eq!(6, drop_id(0));
    assert_eq!(7, drop_id(1));
}

#[test]
fn test_two_tickets_false() {
    reset_drops();
    assert_eq!(6, two_tickets(false));
    assert_eq!(2, n_drops());
    assert_eq!(7, drop_id(0));
    assert_eq!(6, drop_id(1));
}
