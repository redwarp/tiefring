use torchbearer::{
    path::{astar_path, Graph, NodeId, PathMap},
    Point,
};

pub fn find_path<T: PathMap>(map: &T, from: Point, to: Point) -> Option<Vec<Point>> {
    let graph = FourWayGridForceWalkableTarget::new(map, to);
    astar_path(&graph, graph.point_to_index(from), graph.point_to_index(to)).map(|indices| {
        indices
            .into_iter()
            .map(|index| graph.index_to_point(index))
            .collect()
    })
}

/// Variation of the four way grid that assume that the destination is always walkable.
/// Otherwise, a solid player will never be reachable by a monster.
pub struct FourWayGridForceWalkableTarget<'a, T: PathMap> {
    map: &'a T,
    width: i32,
    height: i32,
    to: Point,
}

impl<'a, T: PathMap> FourWayGridForceWalkableTarget<'a, T> {
    pub fn new(map: &'a T, to: Point) -> Self {
        let (width, height) = map.dimensions();
        FourWayGridForceWalkableTarget {
            map,
            width,
            height,
            to,
        }
    }

    /// Is the node at position (x, y) walkable.
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.map.is_walkable((x, y)) || self.to == (x, y)
    }

    fn point_to_index(&self, (x, y): Point) -> usize {
        (x + y * self.width) as usize
    }

    fn index_to_point(&self, index: usize) -> Point {
        (index as i32 % self.width, index as i32 / self.width)
    }
}

impl<'a, T: PathMap> Graph for FourWayGridForceWalkableTarget<'a, T> {
    fn node_count(&self) -> usize {
        (self.width * self.height) as usize
    }

    fn cost_between(&self, a: NodeId, b: NodeId) -> f32 {
        let basic = 1.;
        let (x1, y1) = self.index_to_point(a);
        let (x2, y2) = self.index_to_point(b);
        let nudge = if ((x1 + y1) % 2 == 0 && x2 != x1) || ((x1 + y1) % 2 == 1 && y2 != y1) {
            1.
        } else {
            0.
        };
        basic + 0.001 * nudge
    }

    fn heuristic(&self, a: NodeId, b: NodeId) -> f32 {
        let (xa, ya) = self.index_to_point(a);
        let (xb, yb) = self.index_to_point(b);

        ((xa - xb).abs() + (ya - yb).abs()) as f32
    }

    fn neighboors(&self, a: NodeId, into: &mut Vec<NodeId>) {
        let (x, y) = self.index_to_point(a);

        fn add_to_neighboors_if_qualified<'a, T: PathMap>(
            graph: &FourWayGridForceWalkableTarget<'a, T>,
            (x, y): Point,
            into: &mut Vec<NodeId>,
        ) {
            if x < 0 || y < 0 || x >= graph.width || y >= graph.height || !graph.is_walkable(x, y) {
                return;
            }
            into.push(graph.point_to_index((x, y)));
        }

        add_to_neighboors_if_qualified(self, (x, y + 1), into);
        add_to_neighboors_if_qualified(self, (x, y - 1), into);
        add_to_neighboors_if_qualified(self, (x - 1, y), into);
        add_to_neighboors_if_qualified(self, (x + 1, y), into);
    }
}
