use super::*;
impl<W: DungeonWorld> Placement<'_, W> {
    pub(super) fn building(&mut self) {
        let clear_height = 25;
        self.fill([0, 0, 0], [58, clear_height, 58], WATER, Mode::Flood);
        self.facade(false, 0);
        self.facade(true, 33);
        self.entrance();
        self.arch();
        self.roof();
        self.lower_walls();
        self.middle_walls();
        self.upper_walls();
        for footing_x in 0..7 {
            let mut footing_z = 0;
            while footing_z < 7 {
                if footing_z == 0 && footing_x == 3 {
                    footing_z = 6;
                }
                let base_x = footing_x * 9;
                let base_z = footing_z * 9;
                for dx in 0..4 {
                    for dz in 0..4 {
                        self.block([base_x + dx, 0, base_z + dz], BRICK);
                        self.support([base_x + dx, -1, base_z + dz], BRICK);
                    }
                }
                if footing_x != 0 && footing_x != 6 {
                    footing_z += 6;
                } else {
                    footing_z += 1;
                }
            }
        }
        for ring in 0..5 {
            self.fill(
                [-1 - ring, (ring * 2), -1 - ring],
                [-1 - ring, 23, 58 + ring],
                WATER,
                Mode::Flood,
            );
            self.fill(
                [58 + ring, (ring * 2), -1 - ring],
                [58 + ring, 23, 58 + ring],
                WATER,
                Mode::Flood,
            );
            self.fill(
                [0 - ring, (ring * 2), -1 - ring],
                [57 + ring, 23, -1 - ring],
                WATER,
                Mode::Flood,
            );
            self.fill(
                [0 - ring, (ring * 2), 58 + ring],
                [57 + ring, 23, 58 + ring],
                WATER,
                Mode::Flood,
            );
        }
    }
    fn facade(&mut self, mirror: bool, offset: i32) {
        if self.intersects([offset, 0, offset + 23, 20]) {
            self.fill([offset, 0, 0], [offset + 24, 0, 20], ROUGH, Mode::Solid);
            self.fill([offset, 1, 0], [offset + 24, 10, 20], WATER, Mode::Flood);
            for step in 0..4 {
                self.fill(
                    [offset + step, step + 1, step],
                    [offset + step, step + 1, 20],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [offset + step + 7, step + 5, step + 7],
                    [offset + step + 7, step + 5, 20],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [offset + 17 - step, step + 5, step + 7],
                    [offset + 17 - step, step + 5, 20],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [offset + 24 - step, step + 1, step],
                    [offset + 24 - step, step + 1, 20],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [offset + step + 1, step + 1, step],
                    [offset + 23 - step, step + 1, step],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [offset + step + 8, step + 5, step + 7],
                    [offset + 16 - step, step + 5, step + 7],
                    BRICK,
                    Mode::Solid,
                );
            }
            self.fill([offset + 4, 4, 4], [offset + 6, 4, 20], ROUGH, Mode::Solid);
            self.fill([offset + 7, 4, 4], [offset + 17, 4, 6], ROUGH, Mode::Solid);
            self.fill(
                [offset + 18, 4, 4],
                [offset + 20, 4, 20],
                ROUGH,
                Mode::Solid,
            );
            self.fill(
                [offset + 11, 8, 11],
                [offset + 13, 8, 20],
                ROUGH,
                Mode::Solid,
            );
            self.block([offset + 12, 9, 12], BRICK);
            self.block([offset + 12, 9, 15], BRICK);
            self.block([offset + 12, 9, 18], BRICK);
            let outer_x = if mirror { offset + 19 } else { offset + 5 };
            let inner_x = if mirror { offset + 5 } else { offset + 19 };
            for z in (5..=20).rev().step_by(3) {
                self.block([outer_x, 5, z], BRICK);
            }
            for z in (7..=19).rev().step_by(3) {
                self.block([inner_x, 5, z], BRICK);
            }
            for index in 0..4 {
                let x = if mirror {
                    offset + (24 - (17 - index * 3))
                } else {
                    offset + 17 - index * 3
                };
                self.block([x, 5, 5], BRICK);
            }
            self.block([inner_x, 5, 5], BRICK);
            self.fill(
                [offset + 11, 1, 12],
                [offset + 13, 7, 12],
                ROUGH,
                Mode::Solid,
            );
            self.fill(
                [offset + 12, 1, 11],
                [offset + 12, 7, 13],
                ROUGH,
                Mode::Solid,
            );
        }
    }
    fn entrance(&mut self) {
        if self.intersects([22, 5, 35, 17]) {
            self.fill([25, 0, 0], [32, 8, 20], WATER, Mode::Flood);
            for step in 0..4 {
                self.fill(
                    [24, 2, 5 + step * 4],
                    [24, 4, 5 + step * 4],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [22, 4, 5 + step * 4],
                    [23, 4, 5 + step * 4],
                    BRICK,
                    Mode::Solid,
                );
                self.block([25, 5, 5 + step * 4], BRICK);
                self.block([26, 6, 5 + step * 4], BRICK);
                self.block([26, 5, 5 + step * 4], LANTERN);
                self.fill(
                    [33, 2, 5 + step * 4],
                    [33, 4, 5 + step * 4],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [34, 4, 5 + step * 4],
                    [35, 4, 5 + step * 4],
                    BRICK,
                    Mode::Solid,
                );
                self.block([32, 5, 5 + step * 4], BRICK);
                self.block([31, 6, 5 + step * 4], BRICK);
                self.block([31, 5, 5 + step * 4], LANTERN);
                self.fill(
                    [27, 6, 5 + step * 4],
                    [30, 6, 5 + step * 4],
                    ROUGH,
                    Mode::Solid,
                );
            }
        }
    }
    fn arch(&mut self) {
        if self.intersects([15, 20, 42, 21]) {
            self.fill([15, 0, 21], [42, 0, 21], ROUGH, Mode::Solid);
            self.fill([26, 1, 21], [31, 3, 21], WATER, Mode::Flood);
            self.fill([21, 12, 21], [36, 12, 21], ROUGH, Mode::Solid);
            self.fill([17, 11, 21], [40, 11, 21], ROUGH, Mode::Solid);
            self.fill([16, 10, 21], [41, 10, 21], ROUGH, Mode::Solid);
            self.fill([15, 7, 21], [42, 9, 21], ROUGH, Mode::Solid);
            self.fill([16, 6, 21], [41, 6, 21], ROUGH, Mode::Solid);
            self.fill([17, 5, 21], [40, 5, 21], ROUGH, Mode::Solid);
            self.fill([21, 4, 21], [36, 4, 21], ROUGH, Mode::Solid);
            self.fill([22, 3, 21], [26, 3, 21], ROUGH, Mode::Solid);
            self.fill([31, 3, 21], [35, 3, 21], ROUGH, Mode::Solid);
            self.fill([23, 2, 21], [25, 2, 21], ROUGH, Mode::Solid);
            self.fill([32, 2, 21], [34, 2, 21], ROUGH, Mode::Solid);
            self.fill([28, 4, 20], [29, 4, 21], BRICK, Mode::Solid);
            self.block([27, 3, 21], BRICK);
            self.block([30, 3, 21], BRICK);
            self.block([26, 2, 21], BRICK);
            self.block([31, 2, 21], BRICK);
            self.block([25, 1, 21], BRICK);
            self.block([32, 1, 21], BRICK);
            for step in 0..7 {
                self.block([28 - step, 6 + step, 21], DARK);
                self.block([29 + step, 6 + step, 21], DARK);
            }
            for offset in 0..4 {
                self.block([28 - offset, 9 + offset, 21], DARK);
                self.block([29 + offset, 9 + offset, 21], DARK);
            }
            self.block([28, 12, 21], DARK);
            self.block([29, 12, 21], DARK);
            for inset in 0..3 {
                self.block([22 - inset * 2, 8, 21], DARK);
                self.block([22 - inset * 2, 9, 21], DARK);
                self.block([35 + inset * 2, 8, 21], DARK);
                self.block([35 + inset * 2, 9, 21], DARK);
            }
            self.fill([15, 13, 21], [42, 15, 21], WATER, Mode::Flood);
            self.fill([15, 1, 21], [15, 6, 21], WATER, Mode::Flood);
            self.fill([16, 1, 21], [16, 5, 21], WATER, Mode::Flood);
            self.fill([17, 1, 21], [20, 4, 21], WATER, Mode::Flood);
            self.fill([21, 1, 21], [21, 3, 21], WATER, Mode::Flood);
            self.fill([22, 1, 21], [22, 2, 21], WATER, Mode::Flood);
            self.fill([23, 1, 21], [24, 1, 21], WATER, Mode::Flood);
            self.fill([42, 1, 21], [42, 6, 21], WATER, Mode::Flood);
            self.fill([41, 1, 21], [41, 5, 21], WATER, Mode::Flood);
            self.fill([37, 1, 21], [40, 4, 21], WATER, Mode::Flood);
            self.fill([36, 1, 21], [36, 3, 21], WATER, Mode::Flood);
            self.fill([33, 1, 21], [34, 1, 21], WATER, Mode::Flood);
            self.fill([35, 1, 21], [35, 2, 21], WATER, Mode::Flood);
        }
    }
    fn roof(&mut self) {
        if self.intersects([21, 21, 36, 36]) {
            self.fill([21, 0, 22], [36, 0, 36], ROUGH, Mode::Solid);
            self.fill([21, 1, 22], [36, 23, 36], WATER, Mode::Flood);
            for step in 0..4 {
                self.fill(
                    [21 + step, 13 + step, 21 + step],
                    [36 - step, 13 + step, 21 + step],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [21 + step, 13 + step, 36 - step],
                    [36 - step, 13 + step, 36 - step],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [21 + step, 13 + step, 22 + step],
                    [21 + step, 13 + step, 35 - step],
                    BRICK,
                    Mode::Solid,
                );
                self.fill(
                    [36 - step, 13 + step, 22 + step],
                    [36 - step, 13 + step, 35 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            self.fill([25, 16, 25], [32, 16, 32], ROUGH, Mode::Solid);
            self.fill([25, 17, 25], [25, 19, 25], BRICK, Mode::Solid);
            self.fill([32, 17, 25], [32, 19, 25], BRICK, Mode::Solid);
            self.fill([25, 17, 32], [25, 19, 32], BRICK, Mode::Solid);
            self.fill([32, 17, 32], [32, 19, 32], BRICK, Mode::Solid);
            self.block([26, 20, 26], BRICK);
            self.block([27, 21, 27], BRICK);
            self.block([27, 20, 27], LANTERN);
            self.block([26, 20, 31], BRICK);
            self.block([27, 21, 30], BRICK);
            self.block([27, 20, 30], LANTERN);
            self.block([31, 20, 31], BRICK);
            self.block([30, 21, 30], BRICK);
            self.block([30, 20, 30], LANTERN);
            self.block([31, 20, 26], BRICK);
            self.block([30, 21, 27], BRICK);
            self.block([30, 20, 27], LANTERN);
            self.fill([28, 21, 27], [29, 21, 27], ROUGH, Mode::Solid);
            self.fill([27, 21, 28], [27, 21, 29], ROUGH, Mode::Solid);
            self.fill([28, 21, 30], [29, 21, 30], ROUGH, Mode::Solid);
            self.fill([30, 21, 28], [30, 21, 29], ROUGH, Mode::Solid);
        }
    }
    fn lower_walls(&mut self) {
        if self.intersects([0, 21, 6, 58]) {
            self.fill([0, 0, 21], [6, 0, 57], ROUGH, Mode::Solid);
            self.fill([0, 1, 21], [6, 7, 57], WATER, Mode::Flood);
            self.fill([4, 4, 21], [6, 4, 53], ROUGH, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [step, step + 1, 21],
                    [step, step + 1, 57 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            for z in (23..53).step_by(3) {
                self.block([5, 5, z], BRICK);
            }
            self.block([5, 5, 52], BRICK);
            for step in 0..4 {
                self.fill(
                    [step, step + 1, 21],
                    [step, step + 1, 57 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            self.fill([4, 1, 52], [6, 3, 52], ROUGH, Mode::Solid);
            self.fill([5, 1, 51], [5, 3, 53], ROUGH, Mode::Solid);
        }
        if self.intersects([51, 21, 58, 58]) {
            self.fill([51, 0, 21], [57, 0, 57], ROUGH, Mode::Solid);
            self.fill([51, 1, 21], [57, 7, 57], WATER, Mode::Flood);
            self.fill([51, 4, 21], [53, 4, 53], ROUGH, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [57 - step, step + 1, 21],
                    [57 - step, step + 1, 57 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            for z in (23..53).step_by(3) {
                self.block([52, 5, z], BRICK);
            }
            self.block([52, 5, 52], BRICK);
            self.fill([51, 1, 52], [53, 3, 52], ROUGH, Mode::Solid);
            self.fill([52, 1, 51], [52, 3, 53], ROUGH, Mode::Solid);
        }
        if self.intersects([0, 51, 57, 57]) {
            self.fill([7, 0, 51], [50, 0, 57], ROUGH, Mode::Solid);
            self.fill([7, 1, 51], [50, 10, 57], WATER, Mode::Flood);
            for step in 0..4 {
                self.fill(
                    [step + 1, step + 1, 57 - step],
                    [56 - step, step + 1, 57 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
        }
    }
    fn middle_walls(&mut self) {
        if self.intersects([7, 21, 13, 50]) {
            self.fill([7, 0, 21], [13, 0, 50], ROUGH, Mode::Solid);
            self.fill([7, 1, 21], [13, 10, 50], WATER, Mode::Flood);
            self.fill([11, 8, 21], [13, 8, 53], ROUGH, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [step + 7, step + 5, 21],
                    [step + 7, step + 5, 54],
                    BRICK,
                    Mode::Solid,
                );
            }
            for z in (21..=45).step_by(3) {
                self.block([12, 9, z], BRICK);
            }
        }
        if self.intersects([44, 21, 50, 54]) {
            self.fill([44, 0, 21], [50, 0, 50], ROUGH, Mode::Solid);
            self.fill([44, 1, 21], [50, 10, 50], WATER, Mode::Flood);
            self.fill([44, 8, 21], [46, 8, 53], ROUGH, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [50 - step, step + 5, 21],
                    [50 - step, step + 5, 54],
                    BRICK,
                    Mode::Solid,
                );
            }
            for z in (21..=45).step_by(3) {
                self.block([45, 9, z], BRICK);
            }
        }
        if self.intersects([8, 44, 49, 54]) {
            self.fill([14, 0, 44], [43, 0, 50], ROUGH, Mode::Solid);
            self.fill([14, 1, 44], [43, 10, 50], WATER, Mode::Flood);
            for x in (12..=45).step_by(3) {
                self.block([x, 9, 45], BRICK);
                self.block([x, 9, 52], BRICK);
                if x == 12 || x == 18 || x == 24 || x == 33 || x == 39 || x == 45 {
                    self.block([x, 9, 47], BRICK);
                    self.block([x, 9, 50], BRICK);
                    self.block([x, 10, 45], BRICK);
                    self.block([x, 10, 46], BRICK);
                    self.block([x, 10, 51], BRICK);
                    self.block([x, 10, 52], BRICK);
                    self.block([x, 11, 47], BRICK);
                    self.block([x, 11, 50], BRICK);
                    self.block([x, 12, 48], BRICK);
                    self.block([x, 12, 49], BRICK);
                }
            }
            for step in 0..3 {
                self.fill(
                    [8 + step, 5 + step, 54],
                    [49 - step, 5 + step, 54],
                    ROUGH,
                    Mode::Solid,
                );
            }
            self.fill([11, 8, 54], [46, 8, 54], BRICK, Mode::Solid);
            self.fill([14, 8, 44], [43, 8, 53], ROUGH, Mode::Solid);
        }
    }
    fn upper_walls(&mut self) {
        if self.intersects([14, 21, 20, 43]) {
            self.fill([14, 0, 21], [20, 0, 43], ROUGH, Mode::Solid);
            self.fill([14, 1, 22], [20, 14, 43], WATER, Mode::Flood);
            self.fill([18, 12, 22], [20, 12, 39], ROUGH, Mode::Solid);
            self.fill([18, 12, 21], [20, 12, 21], BRICK, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [step + 14, step + 9, 21],
                    [step + 14, step + 9, 43 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            for z in (23..=39).step_by(3) {
                self.block([19, 13, z], BRICK);
            }
        }
        if self.intersects([37, 21, 43, 43]) {
            self.fill([37, 0, 21], [43, 0, 43], ROUGH, Mode::Solid);
            self.fill([37, 1, 22], [43, 14, 43], WATER, Mode::Flood);
            self.fill([37, 12, 22], [39, 12, 39], ROUGH, Mode::Solid);
            self.fill([37, 12, 21], [39, 12, 21], BRICK, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [43 - step, step + 9, 21],
                    [43 - step, step + 9, 43 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            for z in (23..=39).step_by(3) {
                self.block([38, 13, z], BRICK);
            }
        }
        if self.intersects([15, 37, 42, 43]) {
            self.fill([21, 0, 37], [36, 0, 43], ROUGH, Mode::Solid);
            self.fill([21, 1, 37], [36, 14, 43], WATER, Mode::Flood);
            self.fill([21, 12, 37], [36, 12, 39], ROUGH, Mode::Solid);
            for step in 0..4 {
                self.fill(
                    [15 + step, step + 9, 43 - step],
                    [42 - step, step + 9, 43 - step],
                    BRICK,
                    Mode::Solid,
                );
            }
            for x in (21..=36).step_by(3) {
                self.block([x, 13, 38], BRICK);
            }
        }
    }
    pub(super) fn core(&mut self) {
        self.fill([1, 8, 0], [14, 8, 14], ROUGH, Mode::WaterOnly);
        let ceiling = 7;
        let mut material = BRICK;
        self.fill([0, ceiling, 0], [0, ceiling, 15], material, Mode::Solid);
        self.fill([15, ceiling, 0], [15, ceiling, 15], material, Mode::Solid);
        self.fill([1, ceiling, 0], [15, ceiling, 0], material, Mode::Solid);
        self.fill([1, ceiling, 15], [14, ceiling, 15], material, Mode::Solid);
        for y in 1..=6 {
            material = BRICK;
            if y == 2 || y == 6 {
                material = ROUGH;
            }
            for edge in (0..=15).step_by(15) {
                self.fill([edge, y, 0], [edge, y, 1], material, Mode::Solid);
                self.fill([edge, y, 6], [edge, y, 9], material, Mode::Solid);
                self.fill([edge, y, 14], [edge, y, 15], material, Mode::Solid);
            }
            self.fill([1, y, 0], [1, y, 0], material, Mode::Solid);
            self.fill([6, y, 0], [9, y, 0], material, Mode::Solid);
            self.fill([14, y, 0], [14, y, 0], material, Mode::Solid);
            self.fill([1, y, 15], [14, y, 15], material, Mode::Solid);
        }
        self.fill([6, 3, 6], [9, 6, 9], DARK, Mode::Solid);
        self.fill([7, 4, 7], [8, 5, 8], GOLD, Mode::Solid);
        for offset in (3..=6).step_by(3) {
            for center in (6..=9).step_by(3) {
                self.block([center, offset, 6], LANTERN);
                self.block([center, offset, 9], LANTERN);
            }
        }
        self.fill([5, 1, 6], [5, 2, 6], BRICK, Mode::Solid);
        self.fill([5, 1, 9], [5, 2, 9], BRICK, Mode::Solid);
        self.fill([10, 1, 6], [10, 2, 6], BRICK, Mode::Solid);
        self.fill([10, 1, 9], [10, 2, 9], BRICK, Mode::Solid);
        self.fill([6, 1, 5], [6, 2, 5], BRICK, Mode::Solid);
        self.fill([9, 1, 5], [9, 2, 5], BRICK, Mode::Solid);
        self.fill([6, 1, 10], [6, 2, 10], BRICK, Mode::Solid);
        self.fill([9, 1, 10], [9, 2, 10], BRICK, Mode::Solid);
        self.fill([5, 2, 5], [5, 6, 5], BRICK, Mode::Solid);
        self.fill([5, 2, 10], [5, 6, 10], BRICK, Mode::Solid);
        self.fill([10, 2, 5], [10, 6, 5], BRICK, Mode::Solid);
        self.fill([10, 2, 10], [10, 6, 10], BRICK, Mode::Solid);
        self.fill([5, 7, 1], [5, 7, 6], BRICK, Mode::Solid);
        self.fill([10, 7, 1], [10, 7, 6], BRICK, Mode::Solid);
        self.fill([5, 7, 9], [5, 7, 14], BRICK, Mode::Solid);
        self.fill([10, 7, 9], [10, 7, 14], BRICK, Mode::Solid);
        self.fill([1, 7, 5], [6, 7, 5], BRICK, Mode::Solid);
        self.fill([1, 7, 10], [6, 7, 10], BRICK, Mode::Solid);
        self.fill([9, 7, 5], [14, 7, 5], BRICK, Mode::Solid);
        self.fill([9, 7, 10], [14, 7, 10], BRICK, Mode::Solid);
        self.fill([2, 1, 2], [2, 1, 3], BRICK, Mode::Solid);
        self.fill([3, 1, 2], [3, 1, 2], BRICK, Mode::Solid);
        self.fill([13, 1, 2], [13, 1, 3], BRICK, Mode::Solid);
        self.fill([12, 1, 2], [12, 1, 2], BRICK, Mode::Solid);
        self.fill([2, 1, 12], [2, 1, 13], BRICK, Mode::Solid);
        self.fill([3, 1, 13], [3, 1, 13], BRICK, Mode::Solid);
        self.fill([13, 1, 12], [13, 1, 13], BRICK, Mode::Solid);
        self.fill([12, 1, 13], [12, 1, 13], BRICK, Mode::Solid);
    }
    pub(super) fn double_x(&mut self) {
        let east = self.graph[self.room].neighbor[5];
        let west = self.room;
        if (self.room / 25) > 0 {
            self.floor(8, 0, self.graph[east].open[0]);
            self.floor(0, 0, self.graph[west].open[0]);
        }
        if self.graph[west].neighbor[1] == NONE {
            self.fill([1, 4, 1], [7, 4, 6], ROUGH, Mode::WaterOnly);
        }
        if self.graph[east].neighbor[1] == NONE {
            self.fill([8, 4, 1], [14, 4, 6], ROUGH, Mode::WaterOnly);
        }
        self.fill([0, 3, 0], [0, 3, 7], BRICK, Mode::Solid);
        self.fill([15, 3, 0], [15, 3, 7], BRICK, Mode::Solid);
        self.fill([1, 3, 0], [15, 3, 0], BRICK, Mode::Solid);
        self.fill([1, 3, 7], [14, 3, 7], BRICK, Mode::Solid);
        self.fill([0, 2, 0], [0, 2, 7], ROUGH, Mode::Solid);
        self.fill([15, 2, 0], [15, 2, 7], ROUGH, Mode::Solid);
        self.fill([1, 2, 0], [15, 2, 0], ROUGH, Mode::Solid);
        self.fill([1, 2, 7], [14, 2, 7], ROUGH, Mode::Solid);
        self.fill([0, 1, 0], [0, 1, 7], BRICK, Mode::Solid);
        self.fill([15, 1, 0], [15, 1, 7], BRICK, Mode::Solid);
        self.fill([1, 1, 0], [15, 1, 0], BRICK, Mode::Solid);
        self.fill([1, 1, 7], [14, 1, 7], BRICK, Mode::Solid);
        self.fill([5, 1, 0], [10, 1, 4], BRICK, Mode::Solid);
        self.fill([6, 2, 0], [9, 2, 3], ROUGH, Mode::Solid);
        self.fill([5, 3, 0], [10, 3, 4], BRICK, Mode::Solid);
        self.block([6, 2, 3], LANTERN);
        self.block([9, 2, 3], LANTERN);
        if self.graph[west].open[3] {
            self.fill([3, 1, 0], [4, 2, 0], WATER, Mode::Flood);
        }
        if self.graph[west].open[2] {
            self.fill([3, 1, 7], [4, 2, 7], WATER, Mode::Flood);
        }
        if self.graph[west].open[4] {
            self.fill([0, 1, 3], [0, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[east].open[3] {
            self.fill([11, 1, 0], [12, 2, 0], WATER, Mode::Flood);
        }
        if self.graph[east].open[2] {
            self.fill([11, 1, 7], [12, 2, 7], WATER, Mode::Flood);
        }
        if self.graph[east].open[5] {
            self.fill([15, 1, 3], [15, 2, 4], WATER, Mode::Flood);
        }
    }
    pub(super) fn double_xy(&mut self) {
        let east = self.graph[self.room].neighbor[5];
        let west = self.room;
        let upper_west = self.graph[west].neighbor[1];
        let upper_east = self.graph[east].neighbor[1];
        if (self.room / 25) > 0 {
            self.floor(8, 0, self.graph[east].open[0]);
            self.floor(0, 0, self.graph[west].open[0]);
        }
        if self.graph[upper_west].neighbor[1] == NONE {
            self.fill([1, 8, 1], [7, 8, 6], ROUGH, Mode::WaterOnly);
        }
        if self.graph[upper_east].neighbor[1] == NONE {
            self.fill([8, 8, 1], [14, 8, 6], ROUGH, Mode::WaterOnly);
        }
        for y in 1..=7 {
            let mut material = BRICK;
            if y == 2 || y == 6 {
                material = ROUGH;
            }
            self.fill([0, y, 0], [0, y, 7], material, Mode::Solid);
            self.fill([15, y, 0], [15, y, 7], material, Mode::Solid);
            self.fill([1, y, 0], [15, y, 0], material, Mode::Solid);
            self.fill([1, y, 7], [14, y, 7], material, Mode::Solid);
        }
        self.fill([2, 1, 3], [2, 7, 4], BRICK, Mode::Solid);
        self.fill([3, 1, 2], [4, 7, 2], BRICK, Mode::Solid);
        self.fill([3, 1, 5], [4, 7, 5], BRICK, Mode::Solid);
        self.fill([13, 1, 3], [13, 7, 4], BRICK, Mode::Solid);
        self.fill([11, 1, 2], [12, 7, 2], BRICK, Mode::Solid);
        self.fill([11, 1, 5], [12, 7, 5], BRICK, Mode::Solid);
        self.fill([5, 1, 3], [5, 3, 4], BRICK, Mode::Solid);
        self.fill([10, 1, 3], [10, 3, 4], BRICK, Mode::Solid);
        self.fill([5, 7, 2], [10, 7, 5], BRICK, Mode::Solid);
        self.fill([5, 5, 2], [5, 7, 2], BRICK, Mode::Solid);
        self.fill([10, 5, 2], [10, 7, 2], BRICK, Mode::Solid);
        self.fill([5, 5, 5], [5, 7, 5], BRICK, Mode::Solid);
        self.fill([10, 5, 5], [10, 7, 5], BRICK, Mode::Solid);
        self.block([6, 6, 2], BRICK);
        self.block([9, 6, 2], BRICK);
        self.block([6, 6, 5], BRICK);
        self.block([9, 6, 5], BRICK);
        self.fill([5, 4, 3], [6, 4, 4], BRICK, Mode::Solid);
        self.fill([9, 4, 3], [10, 4, 4], BRICK, Mode::Solid);
        self.block([5, 4, 2], LANTERN);
        self.block([5, 4, 5], LANTERN);
        self.block([10, 4, 2], LANTERN);
        self.block([10, 4, 5], LANTERN);
        if self.graph[west].open[3] {
            self.fill([3, 1, 0], [4, 2, 0], WATER, Mode::Flood);
        }
        if self.graph[west].open[2] {
            self.fill([3, 1, 7], [4, 2, 7], WATER, Mode::Flood);
        }
        if self.graph[west].open[4] {
            self.fill([0, 1, 3], [0, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[east].open[3] {
            self.fill([11, 1, 0], [12, 2, 0], WATER, Mode::Flood);
        }
        if self.graph[east].open[2] {
            self.fill([11, 1, 7], [12, 2, 7], WATER, Mode::Flood);
        }
        if self.graph[east].open[5] {
            self.fill([15, 1, 3], [15, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[upper_west].open[3] {
            self.fill([3, 5, 0], [4, 6, 0], WATER, Mode::Flood);
        }
        if self.graph[upper_west].open[2] {
            self.fill([3, 5, 7], [4, 6, 7], WATER, Mode::Flood);
        }
        if self.graph[upper_west].open[4] {
            self.fill([0, 5, 3], [0, 6, 4], WATER, Mode::Flood);
        }
        if self.graph[upper_east].open[3] {
            self.fill([11, 5, 0], [12, 6, 0], WATER, Mode::Flood);
        }
        if self.graph[upper_east].open[2] {
            self.fill([11, 5, 7], [12, 6, 7], WATER, Mode::Flood);
        }
        if self.graph[upper_east].open[5] {
            self.fill([15, 5, 3], [15, 6, 4], WATER, Mode::Flood);
        }
    }
    pub(super) fn double_y(&mut self) {
        if (self.room / 25) > 0 {
            self.floor(0, 0, self.graph[self.room].open[0]);
        }
        let upper = self.graph[self.room].neighbor[1];
        if self.graph[upper].neighbor[1] == NONE {
            self.fill([1, 8, 1], [6, 8, 6], ROUGH, Mode::WaterOnly);
        }
        self.fill([0, 4, 0], [0, 4, 7], BRICK, Mode::Solid);
        self.fill([7, 4, 0], [7, 4, 7], BRICK, Mode::Solid);
        self.fill([1, 4, 0], [6, 4, 0], BRICK, Mode::Solid);
        self.fill([1, 4, 7], [6, 4, 7], BRICK, Mode::Solid);
        self.fill([2, 4, 1], [2, 4, 2], BRICK, Mode::Solid);
        self.fill([1, 4, 2], [1, 4, 2], BRICK, Mode::Solid);
        self.fill([5, 4, 1], [5, 4, 2], BRICK, Mode::Solid);
        self.fill([6, 4, 2], [6, 4, 2], BRICK, Mode::Solid);
        self.fill([2, 4, 5], [2, 4, 6], BRICK, Mode::Solid);
        self.fill([1, 4, 5], [1, 4, 5], BRICK, Mode::Solid);
        self.fill([5, 4, 5], [5, 4, 6], BRICK, Mode::Solid);
        self.fill([6, 4, 5], [6, 4, 5], BRICK, Mode::Solid);
        let mut level = self.room;
        for base_y in (1..=5).step_by(4) {
            let mut z = 0;
            if self.graph[level].open[3] {
                self.fill([2, base_y, z], [2, base_y + 2, z], BRICK, Mode::Solid);
                self.fill([5, base_y, z], [5, base_y + 2, z], BRICK, Mode::Solid);
                self.fill([3, base_y + 2, z], [4, base_y + 2, z], BRICK, Mode::Solid);
            } else {
                self.fill([0, base_y, z], [7, base_y + 2, z], BRICK, Mode::Solid);
                self.fill([0, base_y + 1, z], [7, base_y + 1, z], ROUGH, Mode::Solid);
            }
            z = 7;
            if self.graph[level].open[2] {
                self.fill([2, base_y, z], [2, base_y + 2, z], BRICK, Mode::Solid);
                self.fill([5, base_y, z], [5, base_y + 2, z], BRICK, Mode::Solid);
                self.fill([3, base_y + 2, z], [4, base_y + 2, z], BRICK, Mode::Solid);
            } else {
                self.fill([0, base_y, z], [7, base_y + 2, z], BRICK, Mode::Solid);
                self.fill([0, base_y + 1, z], [7, base_y + 1, z], ROUGH, Mode::Solid);
            }
            let mut x = 0;
            if self.graph[level].open[4] {
                self.fill([x, base_y, 2], [x, base_y + 2, 2], BRICK, Mode::Solid);
                self.fill([x, base_y, 5], [x, base_y + 2, 5], BRICK, Mode::Solid);
                self.fill([x, base_y + 2, 3], [x, base_y + 2, 4], BRICK, Mode::Solid);
            } else {
                self.fill([x, base_y, 0], [x, base_y + 2, 7], BRICK, Mode::Solid);
                self.fill([x, base_y + 1, 0], [x, base_y + 1, 7], ROUGH, Mode::Solid);
            }
            x = 7;
            if self.graph[level].open[5] {
                self.fill([x, base_y, 2], [x, base_y + 2, 2], BRICK, Mode::Solid);
                self.fill([x, base_y, 5], [x, base_y + 2, 5], BRICK, Mode::Solid);
                self.fill([x, base_y + 2, 3], [x, base_y + 2, 4], BRICK, Mode::Solid);
            } else {
                self.fill([x, base_y, 0], [x, base_y + 2, 7], BRICK, Mode::Solid);
                self.fill([x, base_y + 1, 0], [x, base_y + 1, 7], ROUGH, Mode::Solid);
            }
            level = upper;
        }
    }
    pub(super) fn double_yz(&mut self) {
        let north = self.graph[self.room].neighbor[2];
        let south = self.room;
        let upper_north = self.graph[north].neighbor[1];
        let upper_south = self.graph[south].neighbor[1];
        if (self.room / 25) > 0 {
            self.floor(0, 8, self.graph[north].open[0]);
            self.floor(0, 0, self.graph[south].open[0]);
        }
        if self.graph[upper_south].neighbor[1] == NONE {
            self.fill([1, 8, 1], [6, 8, 7], ROUGH, Mode::WaterOnly);
        }
        if self.graph[upper_north].neighbor[1] == NONE {
            self.fill([1, 8, 8], [6, 8, 14], ROUGH, Mode::WaterOnly);
        }
        for y in 1..=7 {
            let mut material = BRICK;
            if y == 2 || y == 6 {
                material = ROUGH;
            }
            self.fill([0, y, 0], [0, y, 15], material, Mode::Solid);
            self.fill([7, y, 0], [7, y, 15], material, Mode::Solid);
            self.fill([1, y, 0], [6, y, 0], material, Mode::Solid);
            self.fill([1, y, 15], [6, y, 15], material, Mode::Solid);
        }
        for height in 1..=7 {
            let mut trim = DARK;
            if height == 2 || height == 6 {
                trim = LANTERN;
            }
            self.fill([3, height, 7], [4, height, 8], trim, Mode::Solid);
        }
        if self.graph[south].open[3] {
            self.fill([3, 1, 0], [4, 2, 0], WATER, Mode::Flood);
        }
        if self.graph[south].open[5] {
            self.fill([7, 1, 3], [7, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[south].open[4] {
            self.fill([0, 1, 3], [0, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[north].open[2] {
            self.fill([3, 1, 15], [4, 2, 15], WATER, Mode::Flood);
        }
        if self.graph[north].open[4] {
            self.fill([0, 1, 11], [0, 2, 12], WATER, Mode::Flood);
        }
        if self.graph[north].open[5] {
            self.fill([7, 1, 11], [7, 2, 12], WATER, Mode::Flood);
        }
        if self.graph[upper_south].open[3] {
            self.fill([3, 5, 0], [4, 6, 0], WATER, Mode::Flood);
        }
        if self.graph[upper_south].open[5] {
            self.fill([7, 5, 3], [7, 6, 4], WATER, Mode::Flood);
            self.fill([5, 4, 2], [6, 4, 5], BRICK, Mode::Solid);
            self.fill([6, 1, 2], [6, 3, 2], BRICK, Mode::Solid);
            self.fill([6, 1, 5], [6, 3, 5], BRICK, Mode::Solid);
        }
        if self.graph[upper_south].open[4] {
            self.fill([0, 5, 3], [0, 6, 4], WATER, Mode::Flood);
            self.fill([1, 4, 2], [2, 4, 5], BRICK, Mode::Solid);
            self.fill([1, 1, 2], [1, 3, 2], BRICK, Mode::Solid);
            self.fill([1, 1, 5], [1, 3, 5], BRICK, Mode::Solid);
        }
        if self.graph[upper_north].open[2] {
            self.fill([3, 5, 15], [4, 6, 15], WATER, Mode::Flood);
        }
        if self.graph[upper_north].open[4] {
            self.fill([0, 5, 11], [0, 6, 12], WATER, Mode::Flood);
            self.fill([1, 4, 10], [2, 4, 13], BRICK, Mode::Solid);
            self.fill([1, 1, 10], [1, 3, 10], BRICK, Mode::Solid);
            self.fill([1, 1, 13], [1, 3, 13], BRICK, Mode::Solid);
        }
        if self.graph[upper_north].open[5] {
            self.fill([7, 5, 11], [7, 6, 12], WATER, Mode::Flood);
            self.fill([5, 4, 10], [6, 4, 13], BRICK, Mode::Solid);
            self.fill([6, 1, 10], [6, 3, 10], BRICK, Mode::Solid);
            self.fill([6, 1, 13], [6, 3, 13], BRICK, Mode::Solid);
        }
    }
    pub(super) fn double_z(&mut self) {
        let north = self.graph[self.room].neighbor[2];
        let south = self.room;
        if (self.room / 25) > 0 {
            self.floor(0, 8, self.graph[north].open[0]);
            self.floor(0, 0, self.graph[south].open[0]);
        }
        if self.graph[south].neighbor[1] == NONE {
            self.fill([1, 4, 1], [6, 4, 7], ROUGH, Mode::WaterOnly);
        }
        if self.graph[north].neighbor[1] == NONE {
            self.fill([1, 4, 8], [6, 4, 14], ROUGH, Mode::WaterOnly);
        }
        self.fill([0, 3, 0], [0, 3, 15], BRICK, Mode::Solid);
        self.fill([7, 3, 0], [7, 3, 15], BRICK, Mode::Solid);
        self.fill([1, 3, 0], [7, 3, 0], BRICK, Mode::Solid);
        self.fill([1, 3, 15], [6, 3, 15], BRICK, Mode::Solid);
        self.fill([0, 2, 0], [0, 2, 15], ROUGH, Mode::Solid);
        self.fill([7, 2, 0], [7, 2, 15], ROUGH, Mode::Solid);
        self.fill([1, 2, 0], [7, 2, 0], ROUGH, Mode::Solid);
        self.fill([1, 2, 15], [6, 2, 15], ROUGH, Mode::Solid);
        self.fill([0, 1, 0], [0, 1, 15], BRICK, Mode::Solid);
        self.fill([7, 1, 0], [7, 1, 15], BRICK, Mode::Solid);
        self.fill([1, 1, 0], [7, 1, 0], BRICK, Mode::Solid);
        self.fill([1, 1, 15], [6, 1, 15], BRICK, Mode::Solid);
        self.fill([1, 1, 1], [1, 1, 2], BRICK, Mode::Solid);
        self.fill([6, 1, 1], [6, 1, 2], BRICK, Mode::Solid);
        self.fill([1, 3, 1], [1, 3, 2], BRICK, Mode::Solid);
        self.fill([6, 3, 1], [6, 3, 2], BRICK, Mode::Solid);
        self.fill([1, 1, 13], [1, 1, 14], BRICK, Mode::Solid);
        self.fill([6, 1, 13], [6, 1, 14], BRICK, Mode::Solid);
        self.fill([1, 3, 13], [1, 3, 14], BRICK, Mode::Solid);
        self.fill([6, 3, 13], [6, 3, 14], BRICK, Mode::Solid);
        self.fill([2, 1, 6], [2, 3, 6], BRICK, Mode::Solid);
        self.fill([5, 1, 6], [5, 3, 6], BRICK, Mode::Solid);
        self.fill([2, 1, 9], [2, 3, 9], BRICK, Mode::Solid);
        self.fill([5, 1, 9], [5, 3, 9], BRICK, Mode::Solid);
        self.fill([3, 2, 6], [4, 2, 6], BRICK, Mode::Solid);
        self.fill([3, 2, 9], [4, 2, 9], BRICK, Mode::Solid);
        self.fill([2, 2, 7], [2, 2, 8], BRICK, Mode::Solid);
        self.fill([5, 2, 7], [5, 2, 8], BRICK, Mode::Solid);
        self.block([2, 2, 5], LANTERN);
        self.block([5, 2, 5], LANTERN);
        self.block([2, 2, 10], LANTERN);
        self.block([5, 2, 10], LANTERN);
        self.block([2, 3, 5], BRICK);
        self.block([5, 3, 5], BRICK);
        self.block([2, 3, 10], BRICK);
        self.block([5, 3, 10], BRICK);
        if self.graph[south].open[3] {
            self.fill([3, 1, 0], [4, 2, 0], WATER, Mode::Flood);
        }
        if self.graph[south].open[5] {
            self.fill([7, 1, 3], [7, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[south].open[4] {
            self.fill([0, 1, 3], [0, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[north].open[2] {
            self.fill([3, 1, 15], [4, 2, 15], WATER, Mode::Flood);
        }
        if self.graph[north].open[4] {
            self.fill([0, 1, 11], [0, 2, 12], WATER, Mode::Flood);
        }
        if self.graph[north].open[5] {
            self.fill([7, 1, 11], [7, 2, 12], WATER, Mode::Flood);
        }
    }
    pub(super) fn entry(&mut self) {
        self.fill([0, 3, 0], [2, 3, 7], BRICK, Mode::Solid);
        self.fill([5, 3, 0], [7, 3, 7], BRICK, Mode::Solid);
        self.fill([0, 2, 0], [1, 2, 7], BRICK, Mode::Solid);
        self.fill([6, 2, 0], [7, 2, 7], BRICK, Mode::Solid);
        self.fill([0, 1, 0], [0, 1, 7], BRICK, Mode::Solid);
        self.fill([7, 1, 0], [7, 1, 7], BRICK, Mode::Solid);
        self.fill([0, 1, 7], [7, 3, 7], BRICK, Mode::Solid);
        self.fill([1, 1, 0], [2, 3, 0], BRICK, Mode::Solid);
        self.fill([5, 1, 0], [6, 3, 0], BRICK, Mode::Solid);
        if self.graph[self.room].open[2] {
            self.fill([3, 1, 7], [4, 2, 7], WATER, Mode::Flood);
        }
        if self.graph[self.room].open[4] {
            self.fill([0, 1, 3], [1, 2, 4], WATER, Mode::Flood);
        }
        if self.graph[self.room].open[5] {
            self.fill([6, 1, 3], [7, 2, 4], WATER, Mode::Flood);
        }
    }
    pub(super) fn penthouse(&mut self) {
        self.fill([2, -1, 2], [11, -1, 11], BRICK, Mode::Solid);
        self.fill([0, -1, 0], [1, -1, 11], ROUGH, Mode::Solid);
        self.fill([12, -1, 0], [13, -1, 11], ROUGH, Mode::Solid);
        self.fill([2, -1, 0], [11, -1, 1], ROUGH, Mode::Solid);
        self.fill([2, -1, 12], [11, -1, 13], ROUGH, Mode::Solid);
        self.fill([0, 0, 0], [0, 0, 13], BRICK, Mode::Solid);
        self.fill([13, 0, 0], [13, 0, 13], BRICK, Mode::Solid);
        self.fill([1, 0, 0], [12, 0, 0], BRICK, Mode::Solid);
        self.fill([1, 0, 13], [12, 0, 13], BRICK, Mode::Solid);
        for offset in (2..=11).step_by(3) {
            self.block([0, 0, offset], LANTERN);
            self.block([13, 0, offset], LANTERN);
            self.block([offset, 0, 0], LANTERN);
        }
        self.fill([2, 0, 3], [4, 0, 9], BRICK, Mode::Solid);
        self.fill([9, 0, 3], [11, 0, 9], BRICK, Mode::Solid);
        self.fill([4, 0, 9], [9, 0, 11], BRICK, Mode::Solid);
        self.block([5, 0, 8], BRICK);
        self.block([8, 0, 8], BRICK);
        self.block([10, 0, 10], BRICK);
        self.block([3, 0, 10], BRICK);
        self.fill([3, 0, 3], [3, 0, 7], DARK, Mode::Solid);
        self.fill([10, 0, 3], [10, 0, 7], DARK, Mode::Solid);
        self.fill([6, 0, 10], [7, 0, 10], DARK, Mode::Solid);
        let mut x = 3;
        for _var5 in 0..2 {
            for z in (2..=8).step_by(3) {
                self.fill([x, 0, z], [x, 2, z], BRICK, Mode::Solid);
            }
            x = 10;
        }
        self.fill([5, 0, 10], [5, 2, 10], BRICK, Mode::Solid);
        self.fill([8, 0, 10], [8, 2, 10], BRICK, Mode::Solid);
        self.fill([6, -1, 7], [7, -1, 8], DARK, Mode::Solid);
        self.fill([6, -1, 3], [7, -1, 4], WATER, Mode::Flood);
        self.elder([6, 1, 6]);
    }
    pub(super) fn simple(&mut self) {
        if (self.room / 25) > 0 {
            self.floor(0, 0, self.graph[self.room].open[0]);
        }
        if self.graph[self.room].neighbor[1] == NONE {
            self.fill([1, 4, 1], [6, 4, 6], ROUGH, Mode::WaterOnly);
        }
        let pillar = self.variant != 0
            && (self.rng.bits(1) != 0)
            && !self.graph[self.room].open[0]
            && !self.graph[self.room].open[1]
            && self.graph[self.room].open.iter().filter(|v| **v).count() > 1;
        if self.variant == 0 {
            self.fill([0, 1, 0], [2, 1, 2], BRICK, Mode::Solid);
            self.fill([0, 3, 0], [2, 3, 2], BRICK, Mode::Solid);
            self.fill([0, 2, 0], [0, 2, 2], ROUGH, Mode::Solid);
            self.fill([1, 2, 0], [2, 2, 0], ROUGH, Mode::Solid);
            self.block([1, 2, 1], LANTERN);
            self.fill([5, 1, 0], [7, 1, 2], BRICK, Mode::Solid);
            self.fill([5, 3, 0], [7, 3, 2], BRICK, Mode::Solid);
            self.fill([7, 2, 0], [7, 2, 2], ROUGH, Mode::Solid);
            self.fill([5, 2, 0], [6, 2, 0], ROUGH, Mode::Solid);
            self.block([6, 2, 1], LANTERN);
            self.fill([0, 1, 5], [2, 1, 7], BRICK, Mode::Solid);
            self.fill([0, 3, 5], [2, 3, 7], BRICK, Mode::Solid);
            self.fill([0, 2, 5], [0, 2, 7], ROUGH, Mode::Solid);
            self.fill([1, 2, 7], [2, 2, 7], ROUGH, Mode::Solid);
            self.block([1, 2, 6], LANTERN);
            self.fill([5, 1, 5], [7, 1, 7], BRICK, Mode::Solid);
            self.fill([5, 3, 5], [7, 3, 7], BRICK, Mode::Solid);
            self.fill([7, 2, 5], [7, 2, 7], ROUGH, Mode::Solid);
            self.fill([5, 2, 7], [6, 2, 7], ROUGH, Mode::Solid);
            self.block([6, 2, 6], LANTERN);
            if self.graph[self.room].open[3] {
                self.fill([3, 3, 0], [4, 3, 0], BRICK, Mode::Solid);
            } else {
                self.fill([3, 3, 0], [4, 3, 1], BRICK, Mode::Solid);
                self.fill([3, 2, 0], [4, 2, 0], ROUGH, Mode::Solid);
                self.fill([3, 1, 0], [4, 1, 1], BRICK, Mode::Solid);
            }
            if self.graph[self.room].open[2] {
                self.fill([3, 3, 7], [4, 3, 7], BRICK, Mode::Solid);
            } else {
                self.fill([3, 3, 6], [4, 3, 7], BRICK, Mode::Solid);
                self.fill([3, 2, 7], [4, 2, 7], ROUGH, Mode::Solid);
                self.fill([3, 1, 6], [4, 1, 7], BRICK, Mode::Solid);
            }
            if self.graph[self.room].open[4] {
                self.fill([0, 3, 3], [0, 3, 4], BRICK, Mode::Solid);
            } else {
                self.fill([0, 3, 3], [1, 3, 4], BRICK, Mode::Solid);
                self.fill([0, 2, 3], [0, 2, 4], ROUGH, Mode::Solid);
                self.fill([0, 1, 3], [1, 1, 4], BRICK, Mode::Solid);
            }
            if self.graph[self.room].open[5] {
                self.fill([7, 3, 3], [7, 3, 4], BRICK, Mode::Solid);
            } else {
                self.fill([6, 3, 3], [7, 3, 4], BRICK, Mode::Solid);
                self.fill([7, 2, 3], [7, 2, 4], ROUGH, Mode::Solid);
                self.fill([6, 1, 3], [7, 1, 4], BRICK, Mode::Solid);
            }
        } else if self.variant == 1 {
            self.fill([2, 1, 2], [2, 3, 2], BRICK, Mode::Solid);
            self.fill([2, 1, 5], [2, 3, 5], BRICK, Mode::Solid);
            self.fill([5, 1, 5], [5, 3, 5], BRICK, Mode::Solid);
            self.fill([5, 1, 2], [5, 3, 2], BRICK, Mode::Solid);
            self.block([2, 2, 2], LANTERN);
            self.block([2, 2, 5], LANTERN);
            self.block([5, 2, 5], LANTERN);
            self.block([5, 2, 2], LANTERN);
            self.fill([0, 1, 0], [1, 3, 0], BRICK, Mode::Solid);
            self.fill([0, 1, 1], [0, 3, 1], BRICK, Mode::Solid);
            self.fill([0, 1, 7], [1, 3, 7], BRICK, Mode::Solid);
            self.fill([0, 1, 6], [0, 3, 6], BRICK, Mode::Solid);
            self.fill([6, 1, 7], [7, 3, 7], BRICK, Mode::Solid);
            self.fill([7, 1, 6], [7, 3, 6], BRICK, Mode::Solid);
            self.fill([6, 1, 0], [7, 3, 0], BRICK, Mode::Solid);
            self.fill([7, 1, 1], [7, 3, 1], BRICK, Mode::Solid);
            self.block([1, 2, 0], ROUGH);
            self.block([0, 2, 1], ROUGH);
            self.block([1, 2, 7], ROUGH);
            self.block([0, 2, 6], ROUGH);
            self.block([6, 2, 7], ROUGH);
            self.block([7, 2, 6], ROUGH);
            self.block([6, 2, 0], ROUGH);
            self.block([7, 2, 1], ROUGH);
            if !self.graph[self.room].open[3] {
                self.fill([1, 3, 0], [6, 3, 0], BRICK, Mode::Solid);
                self.fill([1, 2, 0], [6, 2, 0], ROUGH, Mode::Solid);
                self.fill([1, 1, 0], [6, 1, 0], BRICK, Mode::Solid);
            }
            if !self.graph[self.room].open[2] {
                self.fill([1, 3, 7], [6, 3, 7], BRICK, Mode::Solid);
                self.fill([1, 2, 7], [6, 2, 7], ROUGH, Mode::Solid);
                self.fill([1, 1, 7], [6, 1, 7], BRICK, Mode::Solid);
            }
            if !self.graph[self.room].open[4] {
                self.fill([0, 3, 1], [0, 3, 6], BRICK, Mode::Solid);
                self.fill([0, 2, 1], [0, 2, 6], ROUGH, Mode::Solid);
                self.fill([0, 1, 1], [0, 1, 6], BRICK, Mode::Solid);
            }
            if !self.graph[self.room].open[5] {
                self.fill([7, 3, 1], [7, 3, 6], BRICK, Mode::Solid);
                self.fill([7, 2, 1], [7, 2, 6], ROUGH, Mode::Solid);
                self.fill([7, 1, 1], [7, 1, 6], BRICK, Mode::Solid);
            }
        } else if self.variant == 2 {
            self.fill([0, 1, 0], [0, 1, 7], BRICK, Mode::Solid);
            self.fill([7, 1, 0], [7, 1, 7], BRICK, Mode::Solid);
            self.fill([1, 1, 0], [6, 1, 0], BRICK, Mode::Solid);
            self.fill([1, 1, 7], [6, 1, 7], BRICK, Mode::Solid);
            self.fill([0, 2, 0], [0, 2, 7], DARK, Mode::Solid);
            self.fill([7, 2, 0], [7, 2, 7], DARK, Mode::Solid);
            self.fill([1, 2, 0], [6, 2, 0], DARK, Mode::Solid);
            self.fill([1, 2, 7], [6, 2, 7], DARK, Mode::Solid);
            self.fill([0, 3, 0], [0, 3, 7], BRICK, Mode::Solid);
            self.fill([7, 3, 0], [7, 3, 7], BRICK, Mode::Solid);
            self.fill([1, 3, 0], [6, 3, 0], BRICK, Mode::Solid);
            self.fill([1, 3, 7], [6, 3, 7], BRICK, Mode::Solid);
            self.fill([0, 1, 3], [0, 2, 4], DARK, Mode::Solid);
            self.fill([7, 1, 3], [7, 2, 4], DARK, Mode::Solid);
            self.fill([3, 1, 0], [4, 2, 0], DARK, Mode::Solid);
            self.fill([3, 1, 7], [4, 2, 7], DARK, Mode::Solid);
            if self.graph[self.room].open[3] {
                self.fill([3, 1, 0], [4, 2, 0], WATER, Mode::Flood);
            }
            if self.graph[self.room].open[2] {
                self.fill([3, 1, 7], [4, 2, 7], WATER, Mode::Flood);
            }
            if self.graph[self.room].open[4] {
                self.fill([0, 1, 3], [0, 2, 4], WATER, Mode::Flood);
            }
            if self.graph[self.room].open[5] {
                self.fill([7, 1, 3], [7, 2, 4], WATER, Mode::Flood);
            }
        }
        if pillar {
            self.fill([3, 1, 3], [4, 1, 4], BRICK, Mode::Solid);
            self.fill([3, 2, 3], [4, 2, 4], ROUGH, Mode::Solid);
            self.fill([3, 3, 3], [4, 3, 4], BRICK, Mode::Solid);
        }
    }
    pub(super) fn sponge(&mut self) {
        if (self.room / 25) > 0 {
            self.floor(0, 0, self.graph[self.room].open[0]);
        }
        if self.graph[self.room].neighbor[1] == NONE {
            self.fill([1, 4, 1], [6, 4, 6], ROUGH, Mode::WaterOnly);
        }
        for x in 1..=6 {
            for z in 1..=6 {
                if self.rng.bounded(3) != 0 {
                    let bottom = if self.rng.bounded(4) == 0 { 2 } else { 3 };
                    self.fill([x, bottom, z], [x, 3, z], SPONGE, Mode::Solid);
                }
            }
        }
        self.fill([0, 1, 0], [0, 1, 7], BRICK, Mode::Solid);
        self.fill([7, 1, 0], [7, 1, 7], BRICK, Mode::Solid);
        self.fill([1, 1, 0], [6, 1, 0], BRICK, Mode::Solid);
        self.fill([1, 1, 7], [6, 1, 7], BRICK, Mode::Solid);
        self.fill([0, 2, 0], [0, 2, 7], DARK, Mode::Solid);
        self.fill([7, 2, 0], [7, 2, 7], DARK, Mode::Solid);
        self.fill([1, 2, 0], [6, 2, 0], DARK, Mode::Solid);
        self.fill([1, 2, 7], [6, 2, 7], DARK, Mode::Solid);
        self.fill([0, 3, 0], [0, 3, 7], BRICK, Mode::Solid);
        self.fill([7, 3, 0], [7, 3, 7], BRICK, Mode::Solid);
        self.fill([1, 3, 0], [6, 3, 0], BRICK, Mode::Solid);
        self.fill([1, 3, 7], [6, 3, 7], BRICK, Mode::Solid);
        self.fill([0, 1, 3], [0, 2, 4], DARK, Mode::Solid);
        self.fill([7, 1, 3], [7, 2, 4], DARK, Mode::Solid);
        self.fill([3, 1, 0], [4, 2, 0], DARK, Mode::Solid);
        self.fill([3, 1, 7], [4, 2, 7], DARK, Mode::Solid);
        if self.graph[self.room].open[3] {
            self.fill([3, 1, 0], [4, 2, 0], WATER, Mode::Flood);
        }
    }
    pub(super) fn wing(&mut self) {
        if self.variant == 0 {
            for step in 0..4 {
                self.fill(
                    [10 - step, 3 - step, 20 - step],
                    [12 + step, 3 - step, 20],
                    BRICK,
                    Mode::Solid,
                );
            }
            self.fill([7, 0, 6], [15, 0, 16], BRICK, Mode::Solid);
            self.fill([6, 0, 6], [6, 3, 20], BRICK, Mode::Solid);
            self.fill([16, 0, 6], [16, 3, 20], BRICK, Mode::Solid);
            self.fill([7, 1, 7], [7, 1, 20], BRICK, Mode::Solid);
            self.fill([15, 1, 7], [15, 1, 20], BRICK, Mode::Solid);
            self.fill([7, 1, 6], [9, 3, 6], BRICK, Mode::Solid);
            self.fill([13, 1, 6], [15, 3, 6], BRICK, Mode::Solid);
            self.fill([8, 1, 7], [9, 1, 7], BRICK, Mode::Solid);
            self.fill([13, 1, 7], [14, 1, 7], BRICK, Mode::Solid);
            self.fill([9, 0, 5], [13, 0, 5], BRICK, Mode::Solid);
            self.fill([10, 0, 7], [12, 0, 7], DARK, Mode::Solid);
            self.fill([8, 0, 10], [8, 0, 12], DARK, Mode::Solid);
            self.fill([14, 0, 10], [14, 0, 12], DARK, Mode::Solid);
            for z in (7..=18).rev().step_by(3) {
                self.block([6, 3, z], LANTERN);
                self.block([16, 3, z], LANTERN);
            }
            self.block([10, 0, 10], LANTERN);
            self.block([12, 0, 10], LANTERN);
            self.block([10, 0, 12], LANTERN);
            self.block([12, 0, 12], LANTERN);
            self.block([8, 3, 6], LANTERN);
            self.block([14, 3, 6], LANTERN);
            self.block([4, 2, 4], BRICK);
            self.block([4, 1, 4], LANTERN);
            self.block([4, 0, 4], BRICK);
            self.block([18, 2, 4], BRICK);
            self.block([18, 1, 4], LANTERN);
            self.block([18, 0, 4], BRICK);
            self.block([4, 2, 18], BRICK);
            self.block([4, 1, 18], LANTERN);
            self.block([4, 0, 18], BRICK);
            self.block([18, 2, 18], BRICK);
            self.block([18, 1, 18], LANTERN);
            self.block([18, 0, 18], BRICK);
            self.block([9, 7, 20], BRICK);
            self.block([13, 7, 20], BRICK);
            self.fill([6, 0, 21], [7, 4, 21], BRICK, Mode::Solid);
            self.fill([15, 0, 21], [16, 4, 21], BRICK, Mode::Solid);
            self.elder([11, 2, 16]);
        } else if self.variant == 1 {
            self.fill([9, 3, 18], [13, 3, 20], BRICK, Mode::Solid);
            self.fill([9, 0, 18], [9, 2, 18], BRICK, Mode::Solid);
            self.fill([13, 0, 18], [13, 2, 18], BRICK, Mode::Solid);
            let mut x = 9;
            let z = 20;
            let y = 5;
            for _var7 in 0..2 {
                self.block([x, y + 1, z], BRICK);
                self.block([x, y, z], LANTERN);
                self.block([x, y - 1, z], BRICK);
                x = 13;
            }
            self.fill([7, 3, 7], [15, 3, 14], BRICK, Mode::Solid);
            x = 10;
            for _var12 in 0..2 {
                self.fill([x, 0, 10], [x, 6, 10], BRICK, Mode::Solid);
                self.fill([x, 0, 12], [x, 6, 12], BRICK, Mode::Solid);
                self.block([x, 0, 10], LANTERN);
                self.block([x, 0, 12], LANTERN);
                self.block([x, 4, 10], LANTERN);
                self.block([x, 4, 12], LANTERN);
                x = 12;
            }
            x = 8;
            for _var13 in 0..2 {
                self.fill([x, 0, 7], [x, 2, 7], BRICK, Mode::Solid);
                self.fill([x, 0, 14], [x, 2, 14], BRICK, Mode::Solid);
                x = 14;
            }
            self.fill([8, 3, 8], [8, 3, 13], DARK, Mode::Solid);
            self.fill([14, 3, 8], [14, 3, 13], DARK, Mode::Solid);
            self.elder([11, 5, 13]);
        }
    }
}
