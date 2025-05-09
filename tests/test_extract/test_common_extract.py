from pathlib import Path
from typing import Final
from unittest.mock import Mock, patch

from fluent.syntax import parse
from fluent.syntax.ast import Identifier, Pattern, Term, TextElement

from ftl_extract.code_extractor import extract_fluent_keys, find_py_files
from ftl_extract.const import (
    DEFAULT_EXCLUDE_DIRS,
    DEFAULT_FTL_FILE,
    DEFAULT_I18N_KEYS,
    DEFAULT_IGNORE_ATTRIBUTES,
    DEFAULT_IGNORE_KWARGS,
)
from ftl_extract.ftl_extractor import extract
from ftl_extract.matcher import FluentKey
from ftl_extract.utils import ExtractionStatistics, prepare_exclude_dirs

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

    self.i18n.prefix.key.one()
    self.i18n.prefix.key.two(_path="content/file.ftl")
    self.i18n.prefix.key.three(arg_1="arg-1", arg_2="arg-2", _path="content/file.ftl")
    self.i18n.prefix.key.four(arg_1=arg1, arg_2=arg2)
    self.i18n.prefix.key.five(arg_1=obj.arg1, arg_2=obj.arg2)
    self.i18n.prefix.key.six(arg_1=obj.arg1(), arg_2=obj.arg2())
"""


def test_common_extract(tmp_path: Path) -> None:
    (tmp_path / "test.py").write_text(CONTENT)

    fluent_keys_len = 30  # Number of keys in `CONTENT`.

    fluent_keys = extract_fluent_keys(
        path=tmp_path,
        i18n_keys=DEFAULT_I18N_KEYS,
        i18n_keys_prefix=("self",),
        exclude_dirs=prepare_exclude_dirs(
            exclude_dirs=DEFAULT_EXCLUDE_DIRS,
            exclude_dirs_append=(),
        ),
        ignore_attributes=DEFAULT_IGNORE_ATTRIBUTES,
        ignore_kwargs=DEFAULT_IGNORE_KWARGS,
        default_ftl_file=DEFAULT_FTL_FILE,
        statistics=ExtractionStatistics(),
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

    assert fluent_keys["prefix-key-one"].key == "prefix-key-one"
    assert fluent_keys["prefix-key-one"].path == Path("_default.ftl")
    assert fluent_keys["prefix-key-one"].translation.value.elements[0].value == "prefix-key-one"
    assert fluent_keys["prefix-key-one"].code_path == tmp_path / "test.py"

    assert fluent_keys["prefix-key-two"].key == "prefix-key-two"
    assert fluent_keys["prefix-key-two"].path == Path("content/file.ftl")
    assert fluent_keys["prefix-key-two"].translation.value.elements[0].value == "prefix-key-two"
    assert fluent_keys["prefix-key-two"].code_path == tmp_path / "test.py"

    assert fluent_keys["prefix-key-three"].key == "prefix-key-three"
    assert fluent_keys["prefix-key-three"].path == Path("content/file.ftl")
    assert fluent_keys["prefix-key-three"].translation.value.elements[0].value == "prefix-key-three"
    assert (
        fluent_keys["prefix-key-three"].translation.value.elements[1].expression.id.name == "arg_1"
    )
    assert (
        fluent_keys["prefix-key-three"].translation.value.elements[2].expression.id.name == "arg_2"
    )
    assert fluent_keys["prefix-key-three"].code_path == tmp_path / "test.py"

    assert fluent_keys["prefix-key-four"].key == "prefix-key-four"
    assert fluent_keys["prefix-key-four"].path == Path("_default.ftl")
    assert fluent_keys["prefix-key-four"].translation.value.elements[0].value == "prefix-key-four"
    assert (
        fluent_keys["prefix-key-four"].translation.value.elements[1].expression.id.name == "arg_1"
    )
    assert (
        fluent_keys["prefix-key-four"].translation.value.elements[2].expression.id.name == "arg_2"
    )
    assert fluent_keys["prefix-key-four"].code_path == tmp_path / "test.py"

    assert fluent_keys["prefix-key-five"].key == "prefix-key-five"
    assert fluent_keys["prefix-key-five"].path == Path("_default.ftl")
    assert fluent_keys["prefix-key-five"].translation.value.elements[0].value == "prefix-key-five"
    assert (
        fluent_keys["prefix-key-five"].translation.value.elements[1].expression.id.name == "arg_1"
    )
    assert (
        fluent_keys["prefix-key-five"].translation.value.elements[2].expression.id.name == "arg_2"
    )
    assert fluent_keys["prefix-key-five"].code_path == tmp_path / "test.py"

    assert fluent_keys["prefix-key-six"].key == "prefix-key-six"
    assert fluent_keys["prefix-key-six"].path == Path("_default.ftl")
    assert fluent_keys["prefix-key-six"].translation.value.elements[0].value == "prefix-key-six"
    assert fluent_keys["prefix-key-six"].translation.value.elements[1].expression.id.name == "arg_1"
    assert fluent_keys["prefix-key-six"].translation.value.elements[2].expression.id.name == "arg_2"
    assert fluent_keys["prefix-key-six"].code_path == tmp_path / "test.py"


def test_extract_fluent_keys_no_files(tmp_path: Path) -> None:
    fluent_keys = extract_fluent_keys(
        path=tmp_path,
        i18n_keys=DEFAULT_I18N_KEYS,
        i18n_keys_prefix=(),
        exclude_dirs=prepare_exclude_dirs(
            exclude_dirs=DEFAULT_EXCLUDE_DIRS,
            exclude_dirs_append=(),
        ),
        ignore_attributes=DEFAULT_IGNORE_ATTRIBUTES,
        ignore_kwargs=DEFAULT_IGNORE_KWARGS,
        default_ftl_file=DEFAULT_FTL_FILE,
        statistics=ExtractionStatistics(),
    )
    assert not fluent_keys


def test_term_paths_are_made_relative_to_output_path(tmp_path: Path) -> None:
    code_path, output_path = (tmp_path / "test.py"), (tmp_path / "output")
    code_path.touch()

    lang = "en"
    mock_term = Mock(FluentKey)
    mock_term.key = "term-1"
    mock_term.path = output_path / lang / "terms.ftl"
    mock_term.translation = Term(
        id=Identifier("term-1"),
        value=Pattern([TextElement(mock_term.key)]),
    )

    with patch(
        "ftl_extract.ftl_extractor.import_ftl_from_dir",
        return_value=({}, {"term-1": mock_term}, []),
    ):
        extract(
            code_path=code_path,
            output_path=output_path,
            language=(lang,),
            i18n_keys=("i18n",),
        )

    assert mock_term.path == Path("terms.ftl")


def test_different_types_of_keys(tmp_path: Path) -> None:
    code_path = Path("tests/files/py/default.py")
    parse(Path("tests/files/locales/en/_default.ftl").read_text(encoding="utf-8"))
    output_path = Path(tmp_path) / "output"
    output_path.mkdir(exist_ok=True)

    statistics = extract(
        code_path=code_path,
        output_path=output_path,
        language=("en",),
        i18n_keys=("i18n",),
        dry_run=True,
    )

    assert all(
        ftl_keys_commented == 0 for ftl_keys_commented in statistics.ftl_keys_commented.values()
    )


def test_find_py_files(tmp_path: Path) -> None:
    # Normal file
    (tmp_path / "test.py").write_text(CONTENT)

    # Dir with file
    (tmp_path / "dir").mkdir()
    (tmp_path / "dir" / "test.py").write_text(CONTENT)

    py_files = find_py_files(
        search_path=tmp_path,
        exclude_dirs=prepare_exclude_dirs(
            exclude_dirs=DEFAULT_EXCLUDE_DIRS,
            exclude_dirs_append=(),
        ),
    )

    assert len(tuple(py_files)) == 2  # noqa: PLR2004


def test_find_py_files_not_file_nor_dir(tmp_path: Path) -> None:
    py_files = find_py_files(
        search_path=tmp_path / "test",
        exclude_dirs=prepare_exclude_dirs(
            exclude_dirs=DEFAULT_EXCLUDE_DIRS,
            exclude_dirs_append=(),
        ),
    )

    assert len(tuple(py_files)) == 0
