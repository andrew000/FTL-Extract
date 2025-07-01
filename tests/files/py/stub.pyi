from contextlib import contextmanager
from typing import Any, Generator, overload

from aiogram_i18n import LazyProxy

class I18nContext(I18nStub):

    def get(self, key: str, /, **kwargs: Any) -> str:
        ...

    async def set_locale(self, locale: str, **kwargs: Any) -> None:
        ...

    @contextmanager
    def use_locale(self, locale: str) -> Generator[I18nContext, None, None]:
        ...

    @contextmanager
    def use_context(self, **kwargs: Any) -> Generator[I18nContext, None, None]:
        ...

    def set_context(self, **kwargs: Any) -> None:
        ...

class LazyFactory(I18nStub):
    key_separator: str

    def set_separator(self, key_separator: str) -> None:
        ...

    def __call__(self, key: str, /, **kwargs: dict[str, Any]) -> LazyProxy:
        ...
L: LazyFactory

class Terms:

    @overload
    def reference(self, *, selector: Any, kwarg1: Any, kwarg2: Any) -> str:
        ...

class Kwargs:
    terms = Terms()

    @overload
    def terms(self, *, selector: Any, kwarg1: Any, kwarg2: Any) -> str:
        ...

class Reference:
    selector = Selector()

class Selector:
    reference = Reference()
    kwargs = Kwargs()

    @overload
    def selectors(self, *, selector: Any) -> str:
        ...

    @overload
    def kwargs(self, *, selector: Any, kwarg1: Any, kwarg2: Any) -> str:
        ...

class Term:

    @overload
    def args(self, *, kwarg1: Any, kwarg2: Any) -> str:
        ...

class MessageReference:

    @overload
    def args(self, *, kwarg1: Any, kwarg2: Any) -> str:
        ...

class Args:
    term = Term()

    @overload
    def term(self) -> str:
        ...

class Text:
    args = Args()
    message_reference = MessageReference()
    selector = Selector()

    @overload
    def message_reference(self) -> str:
        ...

    @overload
    def selector(self, *, selector: Any) -> str:
        ...

    @overload
    def kwargs(self, *, kwarg1: Any, kwarg2: Any) -> str:
        ...

class Get:

    @overload
    def key(self, *, some_kwarg: Any) -> str:
        ...

class Cls:
    get = Get()

    @overload
    def key(self, *, some_kwarg: Any) -> str:
        ...

class Self:
    get = Get()

    @overload
    def key(self, *, some_kwarg: Any) -> str:
        ...

class I18nStub:
    self = Self()
    cls = Cls()
    text = Text()
    message_reference = MessageReference()

    @overload
    def cls(self) -> str:
        ...

    @overload
    def self(self) -> str:
        ...

    @overload
    def message_reference(self) -> str:
        ...

    @overload
    def text(self) -> str:
        ...
