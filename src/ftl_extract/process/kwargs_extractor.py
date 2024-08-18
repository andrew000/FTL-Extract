from fluent.syntax import ast

from ftl_extract.matcher import FluentKey


def _extract_kwargs_from_variable_reference(
    variable_reference: ast.VariableReference,
    kwargs: set[str],
) -> None:
    kwargs.add(variable_reference.id.name)


def _extract_kwargs_from_selector_expression(
    selector_expression: ast.SelectExpression,
    kwargs: set[str],
) -> None:
    if isinstance(selector_expression.selector, ast.VariableReference):
        _extract_kwargs_from_variable_reference(selector_expression.selector, kwargs)

    for variant in selector_expression.variants:
        for placeable in variant.value.elements:
            if isinstance(placeable, ast.Placeable):
                _extract_kwargs_from_placeable(placeable, kwargs)


def _extract_kwargs_from_placeable(placeable: ast.Placeable, kwargs: set[str]) -> None:
    expression = placeable.expression

    if isinstance(expression, ast.VariableReference):
        _extract_kwargs_from_variable_reference(expression, kwargs)

    elif isinstance(expression, ast.SelectExpression):
        _extract_kwargs_from_selector_expression(expression, kwargs)


def extract_kwargs(key: FluentKey) -> set[str]:
    kwargs: set[str] = set()

    if not isinstance(key.translation, ast.Message):
        return kwargs

    if not key.translation.value:
        return kwargs

    for placeable in key.translation.value.elements:
        if isinstance(placeable, ast.Placeable):
            _extract_kwargs_from_placeable(placeable, kwargs)

    return kwargs
