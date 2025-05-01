from pathlib import Path
from typing import Final
from unittest.mock import patch

from fluent.syntax import FluentParser, FluentSerializer, parse

from ftl_extract.ftl_extractor import extract
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

    ftl_keys, _, _, leave_as_is = import_from_ftl(
        path=tmp_path / "test.ftl",
        locale="en",
        parser=FluentParser(with_spans=True),
    )

    serializer = FluentSerializer(with_junk=True)

    comment_ftl_key(key=ftl_keys["key-1"], serializer=serializer)
    comment_ftl_key(key=ftl_keys["key-2"], serializer=serializer)
    comment_ftl_key(key=ftl_keys["key-3"], serializer=serializer)
    comment_ftl_key(key=ftl_keys["key-4"], serializer=serializer)
    comment_ftl_key(key=ftl_keys["key-5"], serializer=serializer)

    ftl, _ = generate_ftl(
        fluent_keys=ftl_keys.values(),
        serializer=serializer,
        leave_as_is=leave_as_is,
    )
    (tmp_path / "test.ftl").write_text(ftl, encoding="utf-8")

    ftl = (tmp_path / "test.ftl").read_text(encoding="utf-8")
    ftl = parse(ftl, with_spans=False)

    assert ftl.body[0].equals(ftl_keys["key-1"].translation)
    assert ftl.body[1].equals(ftl_keys["key-2"].translation)
    assert ftl.body[2].equals(ftl_keys["key-3"].translation)
    assert ftl.body[3].equals(ftl_keys["key-4"].translation)
    assert ftl.body[4].equals(ftl_keys["key-5"].translation)


def test_warn_mode_comments_keys(tmp_path: Path) -> None:
    mock_echo = patch("ftl_extract.ftl_extractor.echo").start()

    code_path = tmp_path / "test_code_path"
    code_path.mkdir()
    (code_path / "code.py").write_text(
        "i18n.get('key-1', var_reference_1='value 1', var_reference_2='value 2')",
    )

    output_path = tmp_path / "test_output_path"
    output_path.mkdir()
    (output_path / "en").mkdir()
    (output_path / "en" / "_default.ftl").write_text(
        "key-1 = Key 1 {$var_reference_1} {$var_reference_2}",
    )

    extract(
        code_path=code_path,
        output_path=output_path,
        language=["en"],
        i18n_keys=["test_key"],
        comment_keys_mode="warn",
    )

    mock_echo.assert_called()
    patch.stopall()
