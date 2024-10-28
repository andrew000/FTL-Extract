from pathlib import Path
from typing import Final

from ftl_extract.code_extractor import extract_fluent_keys
from ftl_extract.const import DEFAULT_FTL_FILE, IGNORE_ATTRIBUTES

CONTENT: Final[str] = """
def test(i18n):
    i18n.get("key-1")
    i18n.get("key-2", _path="content/file.ftl")
    i18n.get("key-3", arg_1="arg-1", arg_2="arg-2", _path="content/file.ftl")
    i18n.get("key-4", arg_1=arg1, arg_2=arg2)
    i18n.get("key-5", arg_1=obj.arg1, arg_2=obj.arg2)
    i18n.get("key-6", arg_1=obj.arg1(), arg_2=obj.arg2())

    i18n.sugar_key_one()
    i18n.sugar_key_two(_path="content/file.ftl")
    i18n.sugar_key_three(arg_1="arg-1", arg_2="arg-2", _path="content/file.ftl")
    i18n.sugar_key_four(arg_1=arg1, arg_2=arg2)
    i18n.sugar_key_five(arg_1=obj.arg1, arg_2=obj.arg2)
    i18n.sugar_key_six(arg_1=obj.arg1(), arg_2=obj.arg2())

    L("lazy-key-1")
    L("lazy-key-2", _path="content/file.ftl")
    L("lazy-key-3", arg_1="arg-1", arg_2="arg-2", _path="content/file.ftl")
    L("lazy-key-4", arg_1=arg1, arg_2=arg2)
    L("lazy-key-5", arg_1=obj.arg1, arg_2=obj.arg2)
    L("lazy-key-6", arg_1=obj.arg1(), arg_2=obj.arg2())

    i18n.attr.key.one()
    i18n.attr.key.two(_path="content/file.ftl")
    i18n.attr.key.three(arg_1="arg-1", arg_2="arg-2", _path="content/file.ftl")
    i18n.attr.key.four(arg_1=arg1, arg_2=arg2)
    i18n.attr.key.five(arg_1=obj.arg1, arg_2=obj.arg2)
    i18n.attr.key.six(arg_1=obj.arg1(), arg_2=obj.arg2())
"""


def test_common_extract(tmp_path: Path) -> None:
    (tmp_path / "test.py").write_text(CONTENT)

    fluent_keys_len = 24  # Number of keys in `CONTENT`.

    fluent_keys = extract_fluent_keys(
        tmp_path,
        ("i18n", "L", "LF"),
        IGNORE_ATTRIBUTES,
        default_ftl_file=DEFAULT_FTL_FILE,
    )
    assert fluent_keys  # Check if `fluent_keys` is not empty.
    assert len(fluent_keys) == fluent_keys_len  # Check if `fluent_keys` has `fluent_keys_len` keys.
    assert "key-1" in fluent_keys
    assert "key-2" in fluent_keys
    assert "key-3" in fluent_keys
    assert "key-4" in fluent_keys
    assert "key-5" in fluent_keys
    assert "key-6" in fluent_keys
    assert "sugar_key_one" in fluent_keys
    assert "sugar_key_two" in fluent_keys
    assert "sugar_key_three" in fluent_keys
    assert "sugar_key_four" in fluent_keys
    assert "sugar_key_five" in fluent_keys
    assert "sugar_key_six" in fluent_keys
    assert "lazy-key-1" in fluent_keys
    assert "lazy-key-2" in fluent_keys
    assert "lazy-key-3" in fluent_keys
    assert "lazy-key-4" in fluent_keys
    assert "lazy-key-5" in fluent_keys
    assert "lazy-key-6" in fluent_keys

    assert fluent_keys["key-1"].key == "key-1"
    assert fluent_keys["key-1"].path == Path("_default.ftl")
    assert fluent_keys["key-1"].translation.value.elements[0].value == "key-1"
    assert fluent_keys["key-1"].code_path == tmp_path / "test.py"

    assert fluent_keys["key-2"].key == "key-2"
    assert fluent_keys["key-2"].path == Path("content/file.ftl")
    assert fluent_keys["key-2"].translation.value.elements[0].value == "key-2"
    assert fluent_keys["key-2"].code_path == tmp_path / "test.py"

    assert fluent_keys["key-3"].key == "key-3"
    assert fluent_keys["key-3"].path == Path("content/file.ftl")
    assert fluent_keys["key-3"].translation.value.elements[0].value == "key-3"
    assert fluent_keys["key-3"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["key-3"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["key-3"].code_path == tmp_path / "test.py"

    assert fluent_keys["key-4"].key == "key-4"
    assert fluent_keys["key-4"].path == Path("_default.ftl")
    assert fluent_keys["key-4"].translation.value.elements[0].value == "key-4"
    assert fluent_keys["key-4"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["key-4"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["key-4"].code_path == tmp_path / "test.py"

    assert fluent_keys["key-5"].key == "key-5"
    assert fluent_keys["key-5"].path == Path("_default.ftl")
    assert fluent_keys["key-5"].translation.value.elements[0].value == "key-5"
    assert fluent_keys["key-5"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["key-5"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["key-5"].code_path == tmp_path / "test.py"

    assert fluent_keys["key-6"].key == "key-6"
    assert fluent_keys["key-6"].path == Path("_default.ftl")
    assert fluent_keys["key-6"].translation.value.elements[0].value == "key-6"
    assert fluent_keys["key-6"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["key-6"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["key-6"].code_path == tmp_path / "test.py"

    assert fluent_keys["sugar_key_one"].key == "sugar_key_one"
    assert fluent_keys["sugar_key_one"].path == Path("_default.ftl")
    assert fluent_keys["sugar_key_one"].translation.value.elements[0].value == "sugar_key_one"
    assert fluent_keys["sugar_key_one"].code_path == tmp_path / "test.py"

    assert fluent_keys["sugar_key_two"].key == "sugar_key_two"
    assert fluent_keys["sugar_key_two"].path == Path("content/file.ftl")
    assert fluent_keys["sugar_key_two"].translation.value.elements[0].value == "sugar_key_two"
    assert fluent_keys["sugar_key_two"].code_path == tmp_path / "test.py"

    assert fluent_keys["sugar_key_three"].key == "sugar_key_three"
    assert fluent_keys["sugar_key_three"].path == Path("content/file.ftl")
    assert fluent_keys["sugar_key_three"].translation.value.elements[0].value == "sugar_key_three"
    assert (
        fluent_keys["sugar_key_three"].translation.value.elements[1].expression.id.name == "arg_1"
    )
    assert (
        fluent_keys["sugar_key_three"].translation.value.elements[2].expression.id.name == "arg_2"
    )
    assert fluent_keys["sugar_key_three"].code_path == tmp_path / "test.py"

    assert fluent_keys["sugar_key_four"].key == "sugar_key_four"
    assert fluent_keys["sugar_key_four"].path == Path("_default.ftl")
    assert fluent_keys["sugar_key_four"].translation.value.elements[0].value == "sugar_key_four"
    assert fluent_keys["sugar_key_four"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["sugar_key_four"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["sugar_key_four"].code_path == tmp_path / "test.py"

    assert fluent_keys["sugar_key_five"].key == "sugar_key_five"
    assert fluent_keys["sugar_key_five"].path == Path("_default.ftl")
    assert fluent_keys["sugar_key_five"].translation.value.elements[0].value == "sugar_key_five"
    assert fluent_keys["sugar_key_five"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["sugar_key_five"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["sugar_key_five"].code_path == tmp_path / "test.py"

    assert fluent_keys["sugar_key_six"].key == "sugar_key_six"
    assert fluent_keys["sugar_key_six"].path == Path("_default.ftl")
    assert fluent_keys["sugar_key_six"].translation.value.elements[0].value == "sugar_key_six"
    assert fluent_keys["sugar_key_six"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["sugar_key_six"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["sugar_key_six"].code_path == tmp_path / "test.py"

    assert fluent_keys["lazy-key-1"].key == "lazy-key-1"
    assert fluent_keys["lazy-key-1"].path == Path("_default.ftl")
    assert fluent_keys["lazy-key-1"].translation.value.elements[0].value == "lazy-key-1"
    assert fluent_keys["lazy-key-1"].code_path == tmp_path / "test.py"

    assert fluent_keys["lazy-key-2"].key == "lazy-key-2"
    assert fluent_keys["lazy-key-2"].path == Path("content/file.ftl")
    assert fluent_keys["lazy-key-2"].translation.value.elements[0].value == "lazy-key-2"
    assert fluent_keys["lazy-key-2"].code_path == tmp_path / "test.py"

    assert fluent_keys["lazy-key-3"].key == "lazy-key-3"
    assert fluent_keys["lazy-key-3"].path == Path("content/file.ftl")
    assert fluent_keys["lazy-key-3"].translation.value.elements[0].value == "lazy-key-3"
    assert fluent_keys["lazy-key-3"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["lazy-key-3"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["lazy-key-3"].code_path == tmp_path / "test.py"

    assert fluent_keys["lazy-key-4"].key == "lazy-key-4"
    assert fluent_keys["lazy-key-4"].path == Path("_default.ftl")
    assert fluent_keys["lazy-key-4"].translation.value.elements[0].value == "lazy-key-4"
    assert fluent_keys["lazy-key-4"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["lazy-key-4"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["lazy-key-4"].code_path == tmp_path / "test.py"

    assert fluent_keys["lazy-key-5"].key == "lazy-key-5"
    assert fluent_keys["lazy-key-5"].path == Path("_default.ftl")
    assert fluent_keys["lazy-key-5"].translation.value.elements[0].value == "lazy-key-5"
    assert fluent_keys["lazy-key-5"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["lazy-key-5"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["lazy-key-5"].code_path == tmp_path / "test.py"

    assert fluent_keys["lazy-key-6"].key == "lazy-key-6"
    assert fluent_keys["lazy-key-6"].path == Path("_default.ftl")
    assert fluent_keys["lazy-key-6"].translation.value.elements[0].value == "lazy-key-6"
    assert fluent_keys["lazy-key-6"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["lazy-key-6"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["lazy-key-6"].code_path == tmp_path / "test.py"

    assert fluent_keys["attr-key-one"].key == "attr-key-one"
    assert fluent_keys["attr-key-one"].path == Path("_default.ftl")
    assert fluent_keys["attr-key-one"].translation.value.elements[0].value == "attr-key-one"
    assert fluent_keys["attr-key-one"].code_path == tmp_path / "test.py"

    assert fluent_keys["attr-key-two"].key == "attr-key-two"
    assert fluent_keys["attr-key-two"].path == Path("content/file.ftl")
    assert fluent_keys["attr-key-two"].translation.value.elements[0].value == "attr-key-two"
    assert fluent_keys["attr-key-two"].code_path == tmp_path / "test.py"

    assert fluent_keys["attr-key-three"].key == "attr-key-three"
    assert fluent_keys["attr-key-three"].path == Path("content/file.ftl")
    assert fluent_keys["attr-key-three"].translation.value.elements[0].value == "attr-key-three"
    assert fluent_keys["attr-key-three"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["attr-key-three"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["attr-key-three"].code_path == tmp_path / "test.py"

    assert fluent_keys["attr-key-four"].key == "attr-key-four"
    assert fluent_keys["attr-key-four"].path == Path("_default.ftl")
    assert fluent_keys["attr-key-four"].translation.value.elements[0].value == "attr-key-four"
    assert fluent_keys["attr-key-four"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["attr-key-four"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["attr-key-four"].code_path == tmp_path / "test.py"

    assert fluent_keys["attr-key-five"].key == "attr-key-five"
    assert fluent_keys["attr-key-five"].path == Path("_default.ftl")
    assert fluent_keys["attr-key-five"].translation.value.elements[0].value == "attr-key-five"
    assert fluent_keys["attr-key-five"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["attr-key-five"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["attr-key-five"].code_path == tmp_path / "test.py"

    assert fluent_keys["attr-key-six"].key == "attr-key-six"
    assert fluent_keys["attr-key-six"].path == Path("_default.ftl")
    assert fluent_keys["attr-key-six"].translation.value.elements[0].value == "attr-key-six"
    assert fluent_keys["attr-key-six"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["attr-key-six"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["attr-key-six"].code_path == tmp_path / "test.py"


def test_extract_fluent_keys_no_files(tmp_path: Path) -> None:
    fluent_keys = extract_fluent_keys(
        tmp_path,
        "i18n",
        IGNORE_ATTRIBUTES,
        default_ftl_file=DEFAULT_FTL_FILE,
    )
    assert not fluent_keys
