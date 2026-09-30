"""Small Rust source lexer for the C10 wiring oracle.

This is deliberately not a Rust parser.  It only provides enough lexical and
scope information to identify the already-approved top-level items. Comments
are omitted, raw identifiers normalize, and literals remain atomic. Token.start preserves the
original source offset needed by diagnostics.
"""
from dataclasses import dataclass


_OPEN = {"(": ")", "[": "]", "{": "}"}
_CLOSE = {value: key for key, value in _OPEN.items()}
_MODIFIERS = {"async", "const", "extern", "unsafe", "default", "move"}
_CONDITIONAL_ATTRIBUTES = {"cfg", "cfg_attr"}


@dataclass(frozen=True)
class Token:
    kind: str
    text: str
    start: int


@dataclass(frozen=True)
class Item:
    kind: str
    name: str
    keyword: int
    header_start: int
    body_open: int | None
    body_close: int | None
    end: int
    conditional: bool


@dataclass(frozen=True)
class Field:
    name: int
    end: int
    conditional: bool
    visible: bool


@dataclass(frozen=True)
class Statement:
    start: int
    end: int | None
    conditional: bool


class SourceError(ValueError):
    """The bounded lexer cannot establish safe delimiter scopes."""


def _block_comment_end(text, start):
    depth = 1
    index = start + 2
    while index < len(text):
        if text.startswith("/*", index):
            depth += 1
            index += 2
        elif text.startswith("*/", index):
            depth -= 1
            index += 2
            if not depth:
                return index
        else:
            index += 1
    return len(text)


def _raw_string_end(text, start):
    if text.startswith(("br", "cr"), start):
        index = start + 2
    elif start < len(text) and text[start] == "r":
        index = start + 1
    else:
        return None
    hashes = 0
    while index < len(text) and text[index] == "#":
        hashes += 1
        index += 1
    if index >= len(text) or text[index] != '"':
        return None
    closing = '"' + ("#" * hashes)
    finish = text.find(closing, index + 1)
    return len(text) if finish < 0 else finish + len(closing)


def _normal_string_quote(text, start):
    if start < len(text) and text[start] == '"':
        return start
    if start + 1 < len(text) and text[start] in "bc" and text[start + 1] == '"':
        return start + 1
    return None


def _normal_string_end(text, quote):
    index = quote + 1
    while index < len(text):
        if text[index] == "\\":
            index += 2
        elif text[index] == '"':
            return index + 1
        else:
            index += 1
    return len(text)


def _char_escape_end(text, start):
    if start + 1 >= len(text):
        return None
    escaped = text[start + 1]
    if escaped == "u" and start + 2 < len(text) and text[start + 2] == "{":
        finish = text.find("}", start + 3)
        return None if finish < 0 else finish + 1
    if escaped == "x":
        finish = start + 4
        if finish <= len(text) and all(character in "0123456789abcdefABCDEF" for character in text[start + 2:finish]):
            return finish
        return None
    return start + 2


def _char_literal_end(text, start):
    quote = start
    if start + 1 < len(text) and text[start] == "b" and text[start + 1] == "'":
        quote += 1
    elif start >= len(text) or text[start] != "'":
        return None
    index = quote + 1
    if index >= len(text) or text[index] in "\r\n":
        return None
    if text[index] == "\\":
        index = _char_escape_end(text, index)
        if index is None:
            return None
    else:
        index += 1
    if index < len(text) and text[index] == "'":
        return index + 1
    # A lifetime such as 'static deliberately reaches this path.  Keeping it
    # in the token stream is important: it is code, not a character literal.
    return None


def _ident_start(character):
    return character == "_" or character.isalpha() or ord(character) >= 128


def _ident_continue(character):
    return character == "_" or character.isalnum() or ord(character) >= 128


def _skip_trivia(text, index):
    while index < len(text):
        if text[index].isspace():
            index += 1
        elif text.startswith("//", index):
            finish = text.find("\n", index + 2)
            index = len(text) if finish < 0 else finish
        elif text.startswith("/*", index):
            index = _block_comment_end(text, index)
        else:
            break
    return index


def tokenize(text):
    """Return code tokens; literals stay atomic and raw identifiers normalize."""
    result = []
    index = int(text.startswith("\ufeff"))
    if text.startswith("#!", index):
        following = _skip_trivia(text, index + 2)
        # Rust distinguishes inner attributes from shebangs after trivia.
        if following == len(text) or text[following] != "[":
            finish = text.find("\n", index)
            index = len(text) if finish < 0 else finish + 1
    while index < len(text):
        index = _skip_trivia(text, index)
        if index == len(text):
            break
        finish = _raw_string_end(text, index)
        if finish is not None:
            result.append(Token("literal", text[index:finish], index))
            index = finish
            continue
        quote = _normal_string_quote(text, index)
        if quote is not None:
            finish = _normal_string_end(text, quote)
            result.append(Token("literal", text[index:finish], index))
            index = finish
            continue
        finish = _char_literal_end(text, index)
        if finish is not None:
            result.append(Token("literal", text[index:finish], index))
            index = finish
            continue
        character = text[index]
        if _ident_start(character):
            identifier = index
            if text.startswith("r#", index) and index + 2 < len(text) and _ident_start(text[index + 2]):
                identifier += 2
            finish = identifier + 1
            while finish < len(text) and _ident_continue(text[finish]):
                finish += 1
            result.append(Token("ident", text[identifier:finish], index))
            index = finish
            continue
        if character.isdigit():
            finish = index + 1
            while finish < len(text) and (_ident_continue(text[finish]) or text[finish] == "."):
                finish += 1
            result.append(Token("number", text[index:finish], index))
            index = finish
            continue
        result.append(Token("punct", character, index))
        index += 1
    return result


def token_texts(text):
    """Return normalized token text, retaining literals but ignoring comments/space."""
    return tuple(token.text for token in tokenize(text))


class RustSource:
    """A delimiter-aware view of only the requested top-level Rust shapes."""

    def __init__(self, text):
        self.tokens = tokenize(text)
        self.matching = {}
        stack = []
        for index, token in enumerate(self.tokens):
            if token.text in _OPEN:
                stack.append(index)
            elif token.text in _CLOSE:
                if not stack or self.tokens[stack[-1]].text != _CLOSE[token.text]:
                    raise SourceError(f"unbalanced delimiter near source offset {token.start}")
                opening = stack.pop()
                self.matching[opening] = index
                self.matching[index] = opening
        if stack:
            raise SourceError("unbalanced delimiter at end of source")
        self.depth_before = []
        depth = 0
        for token in self.tokens:
            self.depth_before.append(depth)
            if token.text in _OPEN:
                depth += 1
            elif token.text in _CLOSE:
                depth -= 1
        self.root_conditional = self._inner_conditional(0, len(self.tokens))

    def values(self, start, end):
        return tuple(token.text for token in self.tokens[start:end])


    def _declaration_start(self, keyword):
        index = keyword - 1
        while index >= 0:
            token = self.tokens[index].text
            if token in _MODIFIERS:
                index -= 1
                continue
            if token == ")" and index in self.matching:
                opening = self.matching[index]
                if opening and self.tokens[opening - 1].text == "pub":
                    return opening - 1
            if token == "pub":
                return index
            if self.tokens[index].kind == "literal" and index and self.tokens[index - 1].text == "extern":
                index -= 1
                continue
            break
        return keyword

    def _attribute_matches(self, opening, closing, names):
        outer = opening > 0 and self.tokens[opening - 1].text == "#"
        inner = opening > 1 and self.values(opening - 2, opening) == ("#", "!")
        if not (outer or inner):
            return False
        for index in range(opening + 1, closing):
            token = self.tokens[index]
            if token.kind == "ident":
                return token.text in names
        return False

    def _inner_conditional(self, start, end):
        """Inspect leading inner attributes on a file or an item body."""
        while start + 2 < end and self.values(start, start + 3) == ("#", "!", "["):
            closing = self.matching[start + 2]
            if self._attribute_matches(start + 2, closing, _CONDITIONAL_ATTRIBUTES):
                return True
            start = closing + 1
        return False

    def conditional_before(self, index, lower=0):
        """Whether contiguous item/statement attributes include cfg/cfg_attr."""
        return self.has_attribute_before(index, _CONDITIONAL_ATTRIBUTES, lower)

    def has_attribute_before(self, index, names, lower=0):
        """Match contiguous declaration attributes, retaining their real scope."""
        cursor = index - 1
        while cursor >= lower:
            token = self.tokens[cursor].text
            if token == "]" and cursor in self.matching:
                opening = self.matching[cursor]
                if self._attribute_matches(opening, cursor, names):
                    return True
                cursor = opening - (3 if self.tokens[opening - 1].text == "!" else 2)
                continue
            if token == ")" and cursor in self.matching:
                opening = self.matching[cursor]
                if opening > lower and self.tokens[opening - 1].text == "pub":
                    cursor = opening - 2
                    continue
            if token in _MODIFIERS or token == "pub":
                cursor -= 1
                continue
            if self.tokens[cursor].kind == "literal" and cursor and self.tokens[cursor - 1].text == "extern":
                cursor -= 1
                continue
            break
        return False

    def _body_open(self, keyword):
        base = self.depth_before[keyword]
        for index in range(keyword + 1, len(self.tokens)):
            depth = self.depth_before[index]
            if depth < base:
                return None
            if depth == base and self.tokens[index].text == ";":
                return None
            if depth == base and self.tokens[index].text == "{":
                return index
        return None

    def _item(self, kind, name, keyword, body_open, end):
        body_close = None if body_open is None else self.matching.get(body_open)
        if body_open is not None and body_close is None:
            raise SourceError(f"missing body close for {kind} {name}")
        return Item(
            kind=kind,
            name=name,
            keyword=keyword,
            header_start=self._declaration_start(keyword),
            body_open=body_open,
            body_close=body_close,
            end=end,
            conditional=(
                self.root_conditional
                or self.conditional_before(keyword)
                or (body_open is not None and self._inner_conditional(body_open + 1, body_close))
            ),
        )

    def top_level_structs(self, name):
        return self._top_level_named("struct", name, require_body=True)

    def top_level_functions(self, name):
        return self._top_level_named("fn", name, require_body=True)

    def top_level_mods(self, name):
        result = []
        for index, token in enumerate(self.tokens):
            if self.depth_before[index] != 0 or token.text != "mod":
                continue
            if index + 1 >= len(self.tokens) or self.tokens[index + 1].text != name:
                continue
            body_open = self._body_open(index)
            if body_open is not None:
                result.append(self._item("mod", name, index, body_open, self.matching[body_open]))
                continue
            for end in range(index + 2, len(self.tokens)):
                if self.depth_before[end] == 0 and self.tokens[end].text == ";":
                    result.append(self._item("mod", name, index, None, end))
                    break
        return result

    def top_level_impls(self, name):
        result = []
        for index, token in enumerate(self.tokens):
            if self.depth_before[index] != 0 or token.text != "impl":
                continue
            if self.values(index + 1, index + 3) != (name, "{"):
                continue
            body_open = index + 2
            result.append(self._item("impl", name, index, body_open, self.matching[body_open]))
        return result

    def _top_level_named(self, kind, name, require_body):
        result = []
        for index, token in enumerate(self.tokens):
            if self.depth_before[index] != 0 or token.text != kind:
                continue
            if index + 1 >= len(self.tokens) or self.tokens[index + 1].text != name:
                continue
            body_open = self._body_open(index)
            if body_open is None and require_body:
                continue
            end = self.matching[body_open] if body_open is not None else index + 1
            result.append(self._item(kind, name, index, body_open, end))
        return result

    def direct_methods(self, scope, name):
        if scope.body_open is None or scope.body_close is None:
            return []
        base = self.depth_before[scope.body_open] + 1
        result = []
        for index in range(scope.body_open + 1, scope.body_close):
            if self.depth_before[index] != base or self.tokens[index].text != "fn":
                continue
            if index + 1 >= scope.body_close or self.tokens[index + 1].text != name:
                continue
            body_open = self._body_open(index)
            if body_open is None or body_open >= scope.body_close:
                continue
            result.append(self._item("fn", name, index, body_open, self.matching[body_open]))
        return result

    def direct_fields(self, scope, name):
        if scope.body_open is None or scope.body_close is None:
            return []
        base = self.depth_before[scope.body_open] + 1
        result = []
        for index in range(scope.body_open + 1, scope.body_close):
            if self.depth_before[index] != base or self.tokens[index].text != name:
                continue
            if index + 1 >= scope.body_close or self.tokens[index + 1].text != ":":
                continue
            end = None
            for probe in range(index + 1, scope.body_close):
                if self.depth_before[probe] == base and self.tokens[probe].text == ",":
                    end = probe
                    break
            if end is None:
                continue
            start = self._direct_boundary(index, scope.body_open, base)
            visible = any(
                self.depth_before[probe] == base and self.tokens[probe].text == "pub"
                for probe in range(start, index)
            )
            result.append(Field(index, end + 1, self.conditional_before(index, start), visible))
        return result

    def direct_assignments(self, scope, receiver, field):
        if scope.body_open is None or scope.body_close is None:
            return []
        base = self.depth_before[scope.body_open] + 1
        result = []
        for index in range(scope.body_open + 1, scope.body_close):
            if self.depth_before[index] != base or self.tokens[index].text != receiver:
                continue
            if index + 3 >= scope.body_close:
                continue
            if self.values(index, index + 4) != (receiver, ".", field, "="):
                continue
            previous = self._previous_direct(index, scope.body_open, base)
            if previous is not None and self.tokens[previous].text not in {";", "{"}:
                continue
            end = None
            for probe in range(index + 4, scope.body_close):
                if self.depth_before[probe] == base and self.tokens[probe].text == ";":
                    end = probe
                    break
            result.append(Statement(index, None if end is None else end + 1, self.conditional_before(index, scope.body_open + 1)))
        return result

    def _previous_direct(self, index, lower, base):
        cursor = index - 1
        while cursor >= lower:
            if self.depth_before[cursor] == base:
                return cursor
            cursor -= 1
        return None

    def _direct_boundary(self, index, lower, base):
        for cursor in range(index - 1, lower, -1):
            if self.depth_before[cursor] == base and self.tokens[cursor].text == ",":
                return cursor + 1
        return lower + 1

    def item_header(self, item):
        end = item.body_open + 1 if item.body_open is not None else item.end + 1
        return self.values(item.header_start, end)

    def item_body(self, item):
        if item.body_open is None or item.body_close is None:
            return ()
        return self.values(item.body_open + 1, item.body_close)


