use coral_engine::{inventory::Inventory, packets::Stack};

fn stone(count: u8) -> Option<Stack> {
    Some(Stack {
        id: 1,
        damage: 0,
        count,
    })
}

#[test]
fn hotbar_swap_then_collect_prefers_partial_stacks() {
    let mut inv = Inventory::default();
    inv.slots[9] = stone(64);
    inv.slots[36] = stone(4);
    assert!(inv.click(9, 0, 2));
    assert_eq!(inv.slots[9], stone(4));
    assert_eq!(inv.slots[36], stone(64));
    inv.cursor = stone(2);
    inv.slots[0] = stone(3);
    inv.slots[1] = stone(64);
    inv.slots[10] = Some(Stack {
        id: 1,
        damage: 1,
        count: 3,
    });
    assert!(!inv.click(11, 9, 2));
    assert!(!inv.click(11, 1, 6));
    assert!(inv.click(11, 0, 6));
    assert_eq!(inv.cursor, stone(64));
    assert_eq!(inv.slots[0], stone(3));
    assert_eq!(inv.slots[1], stone(6));
    assert_eq!(inv.slots[9], None);
    assert_eq!(inv.slots[10].map(|s| s.count), Some(3));
    assert_eq!(inv.slots[36], stone(64));
}
