use crate::packets::Stack;

#[derive(Clone, Copy)]
pub enum Pattern {
    Shaped {
        width: usize,
        height: usize,
        cells: &'static [i16],
    },
    Shapeless(&'static [i16]),
}
pub struct Recipe {
    pub input: Pattern,
    pub output: Stack,
}
const fn item(id: i16, count: u8) -> Stack {
    Stack {
        id,
        count,
        damage: 0,
    }
}
const RECIPES: &[Recipe] = &[
    Recipe {
        input: Pattern::Shapeless(&[17]),
        output: item(5, 4),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 1,
            height: 2,
            cells: &[5, 5],
        },
        output: item(280, 4),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 2,
            height: 2,
            cells: &[5, 5, 5, 5],
        },
        output: item(58, 1),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 3,
            height: 3,
            cells: &[4, 4, 4, 4, 0, 4, 4, 4, 4],
        },
        output: item(61, 1),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 3,
            height: 3,
            cells: &[5, 5, 5, 5, 0, 5, 5, 5, 5],
        },
        output: item(54, 1),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 1,
            height: 3,
            cells: &[4, 4, 280],
        },
        output: item(272, 1),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 1,
            height: 3,
            cells: &[264, 264, 280],
        },
        output: item(276, 1),
    },
    Recipe {
        input: Pattern::Shaped {
            width: 1,
            height: 2,
            cells: &[263, 280],
        },
        output: item(50, 4),
    },
    Recipe {
        input: Pattern::Shapeless(&[289, 12, 289, 12, 289, 12, 289, 12, 289]),
        output: item(46, 1),
    },
];
impl Pattern {
    pub fn matches(self, grid: &[Option<Stack>], size: usize) -> bool {
        if !(2..=3).contains(&size) || grid.len() != size * size {
            return false;
        }
        match self {
            Self::Shapeless(ids) => {
                if grid.iter().flatten().count() != ids.len() || ids.len() > 9 {
                    return false;
                }
                let mut used = 0;
                for item in grid.iter().flatten() {
                    let Some(i) = ids
                        .iter()
                        .enumerate()
                        .position(|(i, id)| used & (1 << i) == 0 && *id == item.id)
                    else {
                        return false;
                    };
                    used |= 1 << i;
                }
                true
            }
            Self::Shaped {
                width,
                height,
                cells,
            } => {
                if width > size
                    || height > size
                    || width == 0
                    || height == 0
                    || cells.len() != width * height
                {
                    return false;
                }
                for dy in 0..=size - height {
                    for dx in 0..=size - width {
                        for mirrored in [false, true] {
                            let matched = (0..grid.len()).all(|i| {
                                let (x, y) = (i % size, i / size);
                                let want =
                                    if x < dx || x >= dx + width || y < dy || y >= dy + height {
                                        0
                                    } else {
                                        let local = x - dx;
                                        cells[(y - dy) * width
                                            + if mirrored { width - 1 - local } else { local }]
                                    };
                                grid[i].map_or(0, |s| s.id) == want
                            });
                            if matched {
                                return true;
                            }
                        }
                    }
                }
                false
            }
        }
    }
}
pub fn craft(grid: &[Option<Stack>], size: usize) -> Option<Stack> {
    RECIPES
        .iter()
        .find(|r| r.input.matches(grid, size))
        .map(|r| {
            let mut out = r.output;
            if out.id == 5 {
                out.damage = grid.iter().flatten().next().unwrap().damage & 3;
            }
            out
        })
}

#[derive(Default)]
pub struct Furnace {
    pub slots: [Option<Stack>; 3],
    pub burn: u16,
    pub fuel_time: u16,
    pub cook: u16,
}
impl Furnace {
    pub fn tick(&mut self) -> bool {
        let before = self.slots;
        self.burn = self.burn.saturating_sub(1);
        let output = self.slots[0].and_then(|s| match s.id {
            4 => Some(1),
            12 => Some(20),
            14 => Some(266),
            15 => Some(265),
            319 => Some(320),
            363 => Some(364),
            365 => Some(366),
            _ => None,
        });
        let Some(id) =
            output.filter(|&id| self.slots[2].is_none_or(|s| s.id == id && s.count < 64))
        else {
            self.cook = 0;
            return false;
        };
        if self.burn == 0
            && let Some(fuel) = self.slots[1]
        {
            self.fuel_time = match fuel.id {
                263 => 1600,
                5 | 17 => 300,
                280 => 100,
                _ => 0,
            };
            self.burn = self.fuel_time;
            if self.burn > 0 {
                consume(&mut self.slots[1]);
            }
        }
        if self.burn > 0 {
            self.cook += 1;
            if self.cook == 200 {
                self.cook = 0;
                consume(&mut self.slots[0]);
                match &mut self.slots[2] {
                    Some(s) => s.count += 1,
                    None => self.slots[2] = Some(item(id, 1)),
                }
            }
        } else {
            self.cook = self.cook.saturating_sub(2);
        }
        self.slots != before
    }
}
pub fn consume(slot: &mut Option<Stack>) {
    if let Some(s) = slot {
        s.count -= 1;
        if s.count == 0 {
            *slot = None;
        }
    }
}
