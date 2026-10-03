"""Regenerate src/compat.rs from the grammers-tl-types schema.

Usage:
    python3 tools/gen_compat.py <path-to-grammers-tl-types-out-dir>

The directory is the cargo build output, e.g.
    target/debug/build/grammers-tl-types-<hash>/out

It contains generated_types.rs / generated_enums.rs /
generated_functions.rs, from which every RPC method's declared return
type is turned into a minimal, protocol-valid TL literal.

Output paths are rewritten from `crate::` to `tl::` because the
generated code lands in this crate, not in grammers_tl_types.
"""
import re, sys, os, json

if len(sys.argv) < 2:
    print(__doc__)
    sys.exit(2)

D = sys.argv[1]
def read(n):
    return open(os.path.join(D, n), encoding='utf-8', errors='replace').read()

ty_src = read('generated_types.rs')
en_src = read('generated_enums.rs')
fn_src = read('generated_functions.rs')

def match_brace(src, i):
    """src[i] == '{'; return index just past matching '}'."""
    assert src[i] == '{', src[max(0,i-40):i+10]
    depth = 0
    j = i
    n = len(src)
    while j < n:
        c = src[j]
        if c == '"':
            j += 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == '\\' else 1
            j += 1
            continue
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    raise ValueError('unbalanced')

def split_top(body):
    """Split a brace/angle/paren-aware comma list at top level."""
    out = []
    depth = 0
    cur = []
    i = 0
    n = len(body)
    while i < n:
        c = body[i]
        if c in '<([{':
            depth += 1
        elif c in '>)]}':
            depth -= 1
        if c == ',' and depth == 0:
            out.append(''.join(cur))
            cur = []
        else:
            cur.append(c)
        i += 1
    if ''.join(cur).strip():
        out.append(''.join(cur))
    return [x.strip() for x in out if x.strip()]

def root_only(src):
    """Blank out every `pub mod X { ... }` body so root scans see only
    top-level items."""
    out = list(src)
    for m in re.finditer(r'^pub mod \w+ \{', src, re.M):
        start = src.index('{', m.start())
        end = match_brace(src, start)
        for i in range(start, end):
            if out[i] != '\n':
                out[i] = ' '
    return ''.join(out)

def parse_mods(src):
    mods = {}
    for m in re.finditer(r'^pub mod (\w+) \{', src, re.M):
        name = m.group(1)
        start = src.index('{', m.start())
        end = match_brace(src, start)
        mods[name] = src[start+1:end-1]
    return mods

ty_mods = parse_mods(ty_src)
en_mods = parse_mods(en_src)
fn_mods = parse_mods(fn_src)

def iter_items(body, kind):
    """Yield (name, inner_body) for `pub struct|enum NAME {` at any indent."""
    for m in re.finditer(r'pub ' + kind + r' (\w+) \{', body):
        start = body.index('{', m.start())
        end = match_brace(body, start)
        yield m.group(1), body[start+1:end-1]

# ---- structs: (mod_or_None, name) -> [(field, type)] ----
structs = {}
for mod, body in list(ty_mods.items()) + [(None, root_only(ty_src))]:
    for name, inner in iter_items(body, 'struct'):
        fields = []
        for part in split_top(inner):
            fm = re.match(r'pub (r#)?(\w+): (.+)$', part, re.S)
            if fm:
                fields.append(((fm.group(1) or '') + fm.group(2), fm.group(3).strip()))
        structs[(mod, name)] = fields

# ---- enums: (mod_or_None, name) -> [(variant, type_path_or_None)] ----
enums = {}
for mod, body in list(en_mods.items()) + [(None, root_only(en_src))]:
    for name, inner in iter_items(body, 'enum'):
        vs = []
        for part in split_top(inner):
            vm = re.match(r'^(\w+)(?:\((crate::types::[\w:]+)\))?$', part, re.S)
            if vm:
                vs.append((vm.group(1), vm.group(2)))
        enums[(mod, name)] = vs

# ---- methods: name -> (ctor, return_type) ----
methods = {}
for mod, body in list(fn_mods.items()) + [(None, root_only(fn_src))]:
    for m in re.finditer(
            r'impl crate::Identifiable for (\w+) \{\s*const CONSTRUCTOR_ID: u32 = (\d+);'
            r'.*?impl crate::RemoteCall for \1 \{\s*type Return = ([^;]+);', body, re.S):
        methods[f'{mod}.{m.group(1)}'] = (int(m.group(2)), m.group(3).strip())

def split_path(path):
    parts = path.split('::')
    if parts[:2] != ['crate', 'types'] and parts[:2] != ['crate', 'enums']:
        return None
    kind = parts[1]
    if len(parts) == 3:
        return kind, None, parts[2]
    if len(parts) == 4:
        return kind, parts[2], parts[3]
    return None

PRIM = {'bool': 'false', 'i32': '0', 'u32': '0', 'i64': '0', 'u64': '0',
        'i8': '0', 'u8': '0', 'i16': '0', 'u16': '0', 'f64': '0.0',
        'String': 'String::new()', 'Vec<u8>': 'Vec::new()'}

PREFERRED = ('Empty', 'NotModified', 'TooLong', 'Disabled', 'None', 'Null')

def enum_expr(path, depth):
    got = split_path(path)
    if not got:
        return None
    _, mod, name = got
    vs = enums.get((mod, name))
    if not vs:
        return None
    def rank(v):
        for i, p in enumerate(PREFERRED):
            if v == p or v.endswith(p):
                return i
        return len(PREFERRED)
    for vname, vtype in sorted(vs, key=lambda kv: rank(kv[0])):
        if vtype is None:
            return f'{path}::{vname}'
        lit = literal(vtype, depth + 1)
        if lit is not None:
            return f'{path}::{vname}({lit})'
    return None

def literal(path, depth=0):
    if depth > 12:
        return None
    path = path.strip()
    if path in PRIM:
        return PRIM[path]
    if path.startswith('Option<'):
        return 'None'
    if path.startswith('Vec<'):
        return 'Vec::new()'
    am = re.fullmatch(r'\[u8; (\d+)\]', path)
    if am:
        return f'[0u8; {am.group(1)}]'
    if path.startswith('Box<'):
        return literal(path[4:-1], depth + 1)
    if path.startswith('crate::enums::'):
        return enum_expr(path, depth)
    if path.startswith('crate::types::'):
        got = split_path(path)
        if not got:
            return None
        _, mod, name = got
        fields = structs.get((mod, name))
        if fields is None:
            return None
        out = []
        for fname, ftype in fields:
            e = literal(ftype, depth + 1)
            if e is None:
                return None
            out.append(f'{fname}: {e}')
        return path + ' { ' + ', '.join(out) + ' }'
    return None

arms = []
unsup = []
for key, (cid, rtype) in sorted(methods.items(), key=lambda kv: kv[1][0]):
    if rtype == 'bool':
        arms.append((cid, key, 'true.to_bytes()'))
        continue
    if rtype.startswith('Vec<'):
        inner = rtype[4:-1].replace('crate::', 'tl::')
        arms.append((cid, key, f'tl::RawVec::<{inner}>(Vec::new()).to_bytes()'))
        continue
    e = literal(rtype)
    if e is None:
        unsup.append((key, rtype))
        continue
    arms.append((cid, key, f"{e.replace('crate::', 'tl::')}.to_bytes()"))

seen = {}
for cid, key, expr in arms:
    seen.setdefault(cid, []).append(key)

print('methods', len(methods), 'arms', len(arms), 'unsupported', len(unsup),
      'distinct_ctors', len(seen))
dups = {k: v for k, v in seen.items() if len(v) > 1}
print('dup_ctors', len(dups))
for k, v in list(dups.items())[:10]:
    print('  DUP', hex(k), v)
for k, t in unsup:
    print('UNSUP', k, t)
json.dump({'arms': arms, 'unsup': unsup}, open('/tmp/arms.json', 'w'))

# ---- emit src/compat.rs ----
header = '''//! Protocol-valid default responses for the broad Telegram API surface.
//!
//! Generated from the `grammers-tl-types` schema: every method below
//! returns a well-formed TL object of its declared return type so that
//! official clients can boot and exercise their full UI against this
//! single-node server. Methods with dedicated handlers in `rpc.rs` are
//! dispatched before this fallback.
//!
//! Deliberately excluded (explicit RPC error instead): payments, phone
//! calls, SMS jobs, premium, fragment and AI compose. Registration is
//! administrator-driven, so `auth.signUp` is rejected rather than stubbed.
//!
//! Regenerate with `tools/gen_compat.py` after upgrading
//! `grammers-tl-types`.

#![allow(clippy::all)]

use grammers_tl_types as tl;
use grammers_tl_types::Serializable as _;

/// Number of methods covered by the generated compatibility surface.
pub const DEFAULT_RESPONSE_METHODS: usize = %d;

/// Namespaces intentionally left unimplemented: they either move real money
/// (payments, premium, fragment), depend on telephony infrastructure (phone,
/// smsjobs) or require a hosted model (aicompose).
pub const UNSUPPORTED_NAMESPACES: &[&str] = &[
    "aicompose",
    "fragment",
    "payments",
    "phone",
    "premium",
    "smsjobs",
];

/// Return a default, well-formed response body for `ctor`, if the method is
/// part of the supported compatibility surface.
pub fn default_response(ctor: u32) -> Option<Vec<u8>> {
    let body = match ctor {
''' % len(arms)

lines = [header]
for cid, key, expr in arms:
    lines.append(f'        {hex(cid)} => {expr},\n')
lines.append('''        _ => return None,
    };
    Some(body)
}
''')

open('src/compat.rs', 'w').write(''.join(lines))
print('wrote src/compat.rs')
