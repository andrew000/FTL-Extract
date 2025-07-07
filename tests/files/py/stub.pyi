from contextlib import contextmanager
from typing import Any, Generator, Literal, overload
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

class I18nStub:
    message_reference = __MessageReference
    text = __Text
    cls = __Cls
    self = __Self

    class __Self:
        get = __Get

        @staticmethod
        def key(*, some_kwarg: Any, **kwargs: Any) -> Literal['self-key{ $some_kwarg }']:
            ...

        class __Get:

            @staticmethod
            def key(*, some_kwarg: Any, **kwargs: Any) -> Literal['self-get-key{ $some_kwarg }']:
                ...

    class __Cls:
        get = __Get

        @staticmethod
        def key(*, some_kwarg: Any, **kwargs: Any) -> Literal['cls-key{ $some_kwarg }']:
            ...

        class __Get:

            @staticmethod
            def key(*, some_kwarg: Any, **kwargs: Any) -> Literal['cls-get-key{ $some_kwarg }']:
                ...

    @staticmethod
    @overload
    def text(**kwargs: Any) -> Literal['This is text']:
        ...

    class __Text:
        selector = __Selector
        message_reference = __MessageReference
        args = __Args

        @staticmethod
        def kwargs(*, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is text with args { $kwarg1 } { $kwarg2 }']:
            ...

        class __Args:
            term = __Term

            @staticmethod
            @overload
            def term(**kwargs: Any) -> Literal['This is text with args as term { -term1 } { -term2 }']:
                ...

            class __Term:

                @staticmethod
                def args(*, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is text with args as term { -term1-with-args } { -term2-with-args }']:
                    ...

        @staticmethod
        @overload
        def message_reference(**kwargs: Any) -> Literal['This is text with another text { message_reference }']:
            ...

        class __MessageReference:

            @staticmethod
            def args(*, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is text with another text { message_reference-args }']:
                ...

        @staticmethod
        @overload
        def selector(*, selector: Any, **kwargs: Any) -> Literal['This is text with selector { $selector ->']:
            ...

        class __Selector:
            reference = __Reference

            @staticmethod
            def selectors(*, selector: Any, **kwargs: Any) -> Literal['This is text with selectors { $selector ->']:
                ...

            @staticmethod
            def kwargs(*, selector: Any, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is text with selector args { $selector ->']:
                ...

            class __Reference:
                selector = __Selector

                class __Selector:
                    kwargs = __Kwargs

                    class __Kwargs:
                        terms = __Terms

                        @staticmethod
                        @overload
                        def terms(*, selector: Any, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is text with selector args { $selector ->']:
                            ...

                        class __Terms:

                            @staticmethod
                            def reference(*, selector: Any, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is text with selector args { $selector ->']:
                                ...

    @staticmethod
    @overload
    def message_reference(**kwargs: Any) -> Literal['This is message_reference, uses as variable for `text-message_reference`']:
        ...

    class __MessageReference:

        @staticmethod
        def args(*, kwarg1: Any, kwarg2: Any, **kwargs: Any) -> Literal['This is message_reference with args { $kwarg1 } { $kwarg2 }, uses as variable for `text-message_reference-args`']:
            ...
