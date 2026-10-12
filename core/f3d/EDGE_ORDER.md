# Edge recipes: the order of the faces round an edge's ends (mitcad#106)

An edge recipe (`edge_recipe_data`, [`design/recipe.rs`](src/design/recipe.rs))
names the two faces along the edge and the faces at its ends. Edges that
lie between the same two faces and have the same faces at their ends
(mirror images of each other: the two edges where a milled flat meets a
shaft, the two where a bore breaks through a face) fit the same names. What
tells them apart is in the bytes after the type string, the recipe's
*tail*: the faces round each end vertex in cyclic order.

## The tail

Little-endian `i32` words (all flags `f` are -1 or 0):

```text
o f | 2, a f, b f | v f |
v × ( k, k × (i f) | c, c × (i f 0 0 f) | d, d × (8 words) | f )
```

- `o` is -1, or the entity of the edge itself where the recipe's first entity
  names the edge (a negative tag; the faces follow it). `a` and `b` are the
  entities of the two faces along the edge.
- `v` is the number of end vertices: 2, or 1 for a closed edge. Per vertex
  (the edge's start first):
  - `k` entity indices, the faces round the vertex **counter-clockwise seen
    from outside the body**, starting at any of them (a rotation is the same
    order). The first two entities are the faces along the edge, the others
    the faces at the ends (the third entity's at the start and the fourth's
    at the end, in the recipes of two ends of three faces each).
  - `c` named edges of the vertex (entity index, framing), as the recipes
    that list the edges of the end faces have them (`-1`, `-102`: the
    negative tags).
  - `d` further groups of eight words (an index, a code and two triples of
    indices): what these say is not known, they are skipped by their framing.
- The flags `f` were 0 where the entity's name carries a negative operation
  number and -1 otherwise in the recipes looked at; their meaning is not used.

All 1,559 distinct tails of the corpora follow this layout.
`design::recipe::edge_tail` reads it; the decoder writes the fans into the
dump as `_f3d.end_faces` (indices into `_f3d.entities`).

## Proof

On the controlled variants (a flat milled on a shaft, a bore breaking through
a block; fillet and chamfer of either twin edge or both) the recipes of the
two twins differ in the order of the end entities and in the fans only, and
the fans are those of the chosen edge: at its start vertex (the vertex at the
start of the ASM edge, the direction of its coedge that is not reversed) the
faces appear in the order the walk of
[`NamedState::edge_endpoint_face_cycles`](src/names.rs) gives against its
sense, at its end vertex in the order it gives. (The walk turns clockwise at
a start vertex and counter-clockwise at an end vertex seen from outside, so
both fans are counter-clockwise.)

On the corpora, 12,961 of the 13,600 edges that the names alone find agree
with their fans at both ends. The others name a face of a fan as an earlier
state of the history had it: the fan's entity is no face of the state the
edge is looked for in (or is another face there), so the order cannot be
compared. Of the 157 fillet and chamfer edge inputs in the 538 designs
that did not fit one edge, 29 now do (a design's earlier versions count
each): the order of the tail picks one of the edges that fit equally, or
the edges fit are one place (a body twice in the state). No edge found
before changed or was lost. In the 321 designs with a reference dump (the
edges of each fillet and chamfer, with their geometry) 27 are new; the 13
that the reference lists are edges the reference selects, not their mirror
images; the others are coincident copies it does not list and edges of an
item it lists only in part.

## Use

`NamedState::edges_fitting_ordered(entities, tail)` does what `edges_fitting`
does and, among the edges that still fit equally, keeps those whose faces
round the ends are in the recipe's order (all of them where none or no
comparable one is: a face of the fan that the state has no entity for stands
for any face, a closed edge or a vertex fan that does not close cannot be
walked). `design::inputs` uses it for an item's edge whose names no other
input of the item has (the n inputs of the same names take the n edges that
fit, in order, as before: each choosing alone, two could take the same
edge), so a twin chosen by the recipe has its geometry in the dump and the
import finds it by that, like any other edge it read.

Still undecided by names and order together (128 of the 157): edges whose
end faces the state names differently than the recipe (the mirror pair of a
hexagonal boss whose end faces were split since), closed edges (the walk
needs two vertices), edges whose vertex fans repeat a face (a seam), edges
between the same two faces that have the same order at both ends (patterned
copies), and the inputs of the same names. The eight words per group `d` may
tell more. `tools/f3d-learn/rules/edge_tail_grammar.py` checks the layout on
a learning dump.

## Inputs that fit the same edges (mitcad#186)

The recipes of the edges of one fillet or chamfer set that lie between the
same two faces (the edges of a ring round a boss, of a free-form patch's
border) can differ in the faces at their ends only, which the state names
alike: each recipe's names fit all the edges, `edges_fitting_ordered` keeps
them all, and the inputs of the same names rule above does not apply as the
names differ. Where the inputs of one edge set that fit the same edges are as
many as those edges, the set is those edges and each input takes one
(`design::inputs::resolve_alike`; which input takes which edge is arbitrary
and does not matter for a set). The edges other inputs of the set already
took are left out of the others' candidates first, and the rule repeats, so
that a few inputs left with as many edges as they are resolve too. Inputs of
different sets share nothing: they are never decided this way.

Checked on the 358 designs with a reference dump (1,963 fillet and chamfer
edge sets, compared by the lengths and middles of their edges): of the 92
inputs the names fit several edges for, 36 are resolved now; 3 sets that
differed agree (two designs of one family and a third), one more set has 18
inputs found that it lacked (the rest of it is not found by names), and no set
that agreed changed. The other 56 are in groups whose size is not the
number of edges that fit them, which this rule does not decide.
