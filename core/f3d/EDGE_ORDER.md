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

## The curves an input stores (mitcad#306, mitcad#106)

An edge input also stores the curve of each edge it stood for when it was
made ([`design/curve.rs`](src/design/curve.rs), the import README's *Stored
edge curves*). Where the names fit several edges, these curves come first,
before the tail's order and the rules above (`design::inputs::pick_edge`):

- **Several edges** (`by_stored_pieces`): the edges the names fit that lie
  on one of the input's stored curves within its ends are the input's, all
  of them, when they are at least two and on one body. An input that stores
  a curve per edge it saw (the two arcs where a chamfer's cone breaks
  through a corner of a hexagon, the pieces of a ring between the same two
  faces) is every edge of the state on those curves; an input that stores
  one curve, which a later change of an earlier item cut in pieces, is
  every piece of it. Edges on one curve count as pieces only when none is
  the whole stored edge and their lengths add up to its length at most (two
  copies of one edge, as where a body is in the state twice, are not two
  pieces). A curve no edge of these names lies on is left alone. The first
  edge is the input's own; the others become edge inputs of their own after
  it in its set (`split_pieces`, with the same `found`, ending `(one of the
  edges its stored curves give)`), once per set: inputs of one set whose
  curves overlap (each input of a ring stores a curve for several of the
  ring's pieces) add an edge the set has once. Only for fillets and
  chamfers, whose sets take any number of edges.
- **One edge** (`by_stored_curve`): the only edge that is the stored edge,
  else the only piece of it.

An input that is still ambiguous says how each edge that fits lies to its
curves (`N edges fit the names (to the stored curves: the stored edge
(curve 1), off the stored curve by 3.2000, …)`).

On the corpora (`MITCAD_F3D_CORPUS`, `MITCAD_F3D_MODELS` and the backup
corpus, 1,020 files, the decoder alone), fillet and chamfer edge inputs that
fit several edges equally, all files counted (distinct files in brackets):

| | inputs ambiguous |
|---|---|
| before mitcad#306 | 92 (56 in 11 files) |
| with the stored curves deciding one edge (mitcad#306) | 86 (50 in 9 files) |
| with several edges (this rule) | 0 |

Every one of the 86 was an input whose two to eight edges each lie on its
stored curves: 83 stored a curve per edge (each edge the stored edge of
another curve; some inputs store curves of edges the names no longer fit
as well), one stored one curve that the state has in eight pieces, two six
curves of which the state has two whole and four in part. The inputs now have 139 edges
more (16,540 → 16,679 edge inputs). Of the edges found before, 30 inputs
now find other ones: inputs of a ring (mitcad#186, *alike*) and inputs of the
same names, which took the edges that fit one each in list order; the set's
edges are the same, each input now with the edges its own curves give.
One input that its names fit in pieces in the state before its item had
been found as a whole edge three states earlier, which the item's bodies no
longer have; it is now its two pieces in its own state. No other input
changed, and none was lost. The edge sets of 25 items (distinct edges per
item) have 195 edges more; one lost the earlier state's edge above.

Eleven of the affected designs imported with the decoder's dumps before and
after: nine fillets and chamfers in seven designs
whose edges the history had to settle (`inferred`) are `read`; no outcome,
no parametric item and no history check changed otherwise. Four items with
inputs found now still fall back as before: the kernel's fillet of their
edges fails. Two (a design and its smaller version) have every edge the history says it took (the 18 edges
where a hexagon's chamfer and rounded corners meet an end face; the body
before them matches the file's state within 1e-5 mm, the edges the replay
finds are the 18 the file's next state lost): the fillet fails on the
file's own geometry, not on upstream noise. Two have edge references the
file has lost.
