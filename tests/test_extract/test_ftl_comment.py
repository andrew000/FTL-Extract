from pathlib import Path
from typing import Final

from fluent.syntax import FluentSerializer, parse

from ftl_extract.ftl_importer import import_from_ftl
from ftl_extract.process.commentator import comment_ftl_key
from ftl_extract.process.serializer import generate_ftl

CONTENT: Final[str] = """
key-1 = Key 1 {$var_reference_1} {$var_reference_2}
key-2 = Key 2 {msg_reference_1} {msg_reference_2}
key-3 = Key 3
key-4 = ⚙️ Header Text: { $selected_type ->
    [text] { set-type-text-button }
    [photo] { set-type-photo-button }
    [video] { set-type-video-button }
    [gif] { set-type-gif-button }
    [sticker] { set-type-sticker-button }
    *[unknown] 🤷‍♂️
}
key-5 = ⚠️ Header Text. Header Text.

    { chat_settings }

    💁‍♂️ Text inside <code>Continent/City</code>

    💡 Text inside:
        <blockquote><code>indent</code></blockquote>
      <blockquote><code>no indent</code></blockquote>
"""


def test_ftl_comment(tmp_path: Path) -> None:
    (tmp_path / "test.ftl").write_text(CONTENT, encoding="utf-8")

    ftl_keys, resource = import_from_ftl(tmp_path / "test.ftl", "en")

    serializer = FluentSerializer(with_junk=True)

    comment_ftl_key(ftl_keys["key-1"], serializer=serializer)
    comment_ftl_key(ftl_keys["key-2"], serializer=serializer)
    comment_ftl_key(ftl_keys["key-3"], serializer=serializer)
    comment_ftl_key(ftl_keys["key-4"], serializer=serializer)
    comment_ftl_key(ftl_keys["key-5"], serializer=serializer)

    ftl, _ = generate_ftl(ftl_keys, serializer=serializer)
    (tmp_path / "test.ftl").write_text(ftl, encoding="utf-8")

    ftl = (tmp_path / "test.ftl").read_text(encoding="utf-8")
    ftl = parse(ftl, with_spans=False)

    assert ftl.body[0].equals(ftl_keys["key-1"].translation)
    assert ftl.body[1].equals(ftl_keys["key-2"].translation)
    assert ftl.body[2].equals(ftl_keys["key-3"].translation)
    assert ftl.body[3].equals(ftl_keys["key-4"].translation)
    assert ftl.body[4].equals(ftl_keys["key-5"].translation)
