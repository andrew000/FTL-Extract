from typing import Any


# Stub
class I18nContext:
    def get(self, *_, **__) -> None: ...

    def __getattr__(self, item: str) -> Any: ...

    def __call__(self, *_, **__) -> None: ...


i18n = I18nContext()

# Text
i18n.text()

# Text with arguments
i18n.text.kwargs(kwarg1="value1", kwarg2="value2")

# Text with arguments as term
i18n.text.args.term()

# Text with arguments as term with arguments in terms
i18n.text.args.term.args(kwarg1="value1", kwarg2="value2")

# Text with message_reference
i18n.text.message_reference()

# Text with message_reference with arguments
i18n.text.message_reference.args(kwarg1="value1", kwarg2="value2")

# Text with selector
i18n.text.selector(selector=1)

# Text with selector with selectors
i18n.text.selector.selectors(selector=1)

# Text with selector with arguments
i18n.text.selector.kwargs(selector=1, kwarg1="value1", kwarg2="value2")

# Text with selector with selectors and arguments
i18n.text.selector.reference.selector.kwargs.terms(
    selector=1,
    kwarg1="value1",
    kwarg2="value2",
)
