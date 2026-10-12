# SPDX-License-Identifier: MIT
"""learn.py rule: every edge recipe's tail follows the layout of
core/f3d/EDGE_ORDER.md (mitcad#106).

    learn.py test --kind FilletFeature --rule tools/f3d-learn/rules/edge_tail_grammar.py --unsettled
    learn.py test --kind ChamferFeature --rule ... --unsettled

check(ex) is True when the item has edge recipes and all their tails parse
(the entity indices of the fans and of the two faces lie in the recipe's
entity list, the fans' faces are among the first entities or the ends'),
False when one does not, None when the dump has no tail framing for any.
"""


def words(tail_hex):
    b = bytes.fromhex(tail_hex)
    if len(b) % 4:
        return None
    return [int.from_bytes(b[i:i + 4], "little", signed=True) for i in range(0, len(b), 4)]


def parse(w):
    """(faces, fans) of a tail's words, or None."""
    n = len(w)

    def at(p):
        return w[p] if 0 <= p < n else None

    def flag(p):
        return at(p) in (-1, 0)

    if at(0) is None or at(0) < -1 or not flag(1) or at(2) != 2 or not flag(4) or not flag(6):
        return None
    faces = (at(3), at(5))
    v = at(7)
    if v is None or v < 0 or v > 2 or not flag(8):
        return None
    p = 9
    fans = []
    for _ in range(v):
        k = at(p)
        if k is None or k < 0:
            return None
        p += 1
        fan = []
        for _ in range(k):
            if at(p) is None or at(p) < 0 or not flag(p + 1):
                return None
            fan.append(at(p))
            p += 2
        c = at(p)
        if c is None or c < 0:
            return None
        p += 1
        for _ in range(c):
            if (at(p) is None or at(p) < 0 or not flag(p + 1) or at(p + 2) != 0
                    or at(p + 3) != 0 or not flag(p + 4)):
                return None
            p += 5
        d = at(p)
        if d is None or d < 0:
            return None
        p += 1 + 8 * d
        if not flag(p):
            return None
        p += 1
        fans.append(fan)
    return (faces, fans) if p == n else None


def check(ex):
    seen = False
    for o in ex.objects.values():
        r = o.recipe
        if r.get("kind") != "edge" or "tail_hex" not in r:
            continue
        seen = True
        w = words(r["tail_hex"])
        parsed = parse(w) if w is not None else None
        if parsed is None:
            return False
        faces, fans = parsed
        n = len(r.get("entities", []))
        if any(i >= n for i in faces) or any(i >= n for fan in fans for i in fan):
            return False
    return True if seen else None
