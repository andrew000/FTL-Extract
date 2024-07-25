from fluent.syntax import ast

from ftl_extract.matcher import FluentKey


def extract_kwargs(key: FluentKey) -> set[str]:
    kwargs: set[str] = set()

    if not isinstance(key.translation, ast.Message):
        return kwargs

    if not key.translation.value:
        return kwargs

    for placeable in key.translation.value.elements:
        if isinstance(placeable, ast.Placeable):
            expression = placeable.expression

            if isinstance(expression, ast.VariableReference):
                kwargs.add(expression.id.name)

            elif isinstance(expression, ast.SelectExpression) and isinstance(
                expression.selector, ast.VariableReference
            ):
                kwargs.add(expression.selector.id.name)

    return kwargs
