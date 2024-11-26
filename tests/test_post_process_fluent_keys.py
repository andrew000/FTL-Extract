from pathlib import Path
from unittest.mock import Mock

from ftl_extract.code_extractor import post_process_fluent_keys
from ftl_extract.const import DEFAULT_FTL_FILE
from ftl_extract.matcher import FluentKey


def test_process_fluent_key() -> None:
    fluent_mock = Mock(spec=FluentKey)
    fluent_mock.path = "test.ftl"
    fluent_keys = {"key-1": fluent_mock}

    post_process_fluent_keys(fluent_keys=fluent_keys, default_ftl_file=DEFAULT_FTL_FILE)
    assert fluent_mock.path == Path("test.ftl")


def test_process_fluent_key_default() -> None:
    fluent_mock = Mock(spec=FluentKey)
    fluent_mock.path = Path("test")
    fluent_keys = {"key-1": fluent_mock}

    post_process_fluent_keys(fluent_keys=fluent_keys, default_ftl_file=DEFAULT_FTL_FILE)
    assert fluent_mock.path == Path("test/_default.ftl")
