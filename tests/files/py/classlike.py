# Stub
from typing import Any


class I18nContext:
    def get(self, *_, **__) -> None: ...

    def __getattr__(self, item: str) -> Any: ...

    def __call__(self, *_, **__) -> None: ...


class Mock:
    cls_i18n: I18nContext

    def __init__(self, i18n: I18nContext) -> None:
        self.i18n = i18n

    def self_i18n(self) -> None:
        self.i18n.self.key(some_kwarg="...", _path="classlike.ftl")
        self.i18n.get("self-get-key", some_kwarg="...", _path="classlike.ftl")

    @classmethod
    def cls_i18n(cls) -> None:
        cls.cls_i18n.cls.key(some_kwarg="...", _path="classlike.ftl")
        cls.cls_i18n.get("cls-get-key", some_kwarg="...", _path="classlike.ftl")
