from __future__ import annotations

import ast
from _ast import stmt
from collections.abc import Iterable
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, cast

from fluent.syntax import FluentParser

from ftl_extract.stub.node import create_node
from ftl_extract.stub.utils import to_camel_case
from ftl_extract.stub.visitor import FluentVisitor, Message

if TYPE_CHECKING:
    from collections.abc import Generator
    from pathlib import Path
    from typing import Any

    from ftl_extract.stub.node import Node


class NoBodyError(Exception): ...


class BuildASTUndefinedBehaviorError(Exception): ...


@dataclass(unsafe_hash=True)
class FunctionDefWrapper:
    name: str
    function_def: ast.FunctionDef = field(hash=False, compare=False)
    args: tuple[str, ...] = field(default_factory=tuple)


@dataclass
class ClassDefWrapper:
    name: str
    class_def: ast.ClassDef
    attributes: list[str] = field(default_factory=list)
    methods: set[FunctionDefWrapper] = field(default_factory=set)


def read_ftl_messages(visitor: FluentVisitor, path: Path) -> dict[str, Message]:
    resource = FluentParser().parse(path.read_text())
    if not resource.body:
        msg = "no body"
        raise NoBodyError(msg)

    visitor.visit(resource)

    return visitor.messages


def locate_ftl_files(path: Path) -> Generator[Path, Any, Any]:
    if path.is_dir():
        yield from path.rglob("*.ftl")
    else:
        yield from (path,)


def build_base_ast() -> ast.Module:
    return ast.Module(
        body=[
            ast.ImportFrom(module="contextlib", names=[ast.alias(name="contextmanager")], level=0),
            ast.ImportFrom(
                module="typing",
                names=[
                    ast.alias(name="Any"),
                    ast.alias(name="Generator"),
                    ast.alias(name="overload"),
                ],
                level=0,
            ),
            ast.ImportFrom(module="aiogram_i18n", names=[ast.alias(name="LazyProxy")], level=0),
            ast.ClassDef(
                name="I18nContext",
                bases=[ast.Name(id="I18nStub")],
                keywords=[],
                body=[
                    ast.FunctionDef(
                        name="get",
                        args=ast.arguments(
                            posonlyargs=[
                                ast.arg(arg="self"),
                                ast.arg(arg="key", annotation=ast.Name(id="str")),
                            ],
                            args=[],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=ast.arg(arg="kwargs", annotation=ast.Name(id="Any")),
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[],
                        returns=ast.Name(id="str"),
                    ),
                    ast.AsyncFunctionDef(
                        name="set_locale",
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[
                                ast.arg(arg="self"),
                                ast.arg(arg="locale", annotation=ast.Name(id="str")),
                            ],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=ast.arg(arg="kwargs", annotation=ast.Name(id="Any")),
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[],
                        returns=ast.Constant(value=None),
                    ),
                    ast.FunctionDef(
                        name="use_locale",
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[
                                ast.arg(arg="self"),
                                ast.arg(arg="locale", annotation=ast.Name(id="str")),
                            ],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=None,
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[ast.Name(id="contextmanager")],
                        returns=ast.Subscript(
                            value=ast.Name(id="Generator"),
                            slice=ast.Tuple(
                                elts=[
                                    ast.Name(id="I18nContext"),
                                    ast.Constant(value=None),
                                    ast.Constant(value=None),
                                ],
                            ),
                        ),
                    ),
                    ast.FunctionDef(
                        name="use_context",
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[ast.arg(arg="self")],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=ast.arg(arg="kwargs", annotation=ast.Name(id="Any")),
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[ast.Name(id="contextmanager")],
                        returns=ast.Subscript(
                            value=ast.Name(id="Generator"),
                            slice=ast.Tuple(
                                elts=[
                                    ast.Name(id="I18nContext"),
                                    ast.Constant(value=None),
                                    ast.Constant(value=None),
                                ],
                            ),
                        ),
                    ),
                    ast.FunctionDef(
                        name="set_context",
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[ast.arg(arg="self")],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=ast.arg(arg="kwargs", annotation=ast.Name(id="Any")),
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[],
                        returns=ast.Constant(value=None),
                    ),
                ],
                decorator_list=[],
            ),
            ast.ClassDef(
                name="LazyFactory",
                bases=[ast.Name(id="I18nStub")],
                keywords=[],
                body=[
                    ast.AnnAssign(
                        target=ast.Name(id="key_separator"),
                        annotation=ast.Name(id="str"),
                        simple=1,
                    ),
                    ast.FunctionDef(
                        name="set_separator",
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[
                                ast.arg(arg="self"),
                                ast.arg(arg="key_separator", annotation=ast.Name(id="str")),
                            ],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=None,
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[],
                        returns=ast.Constant(value=None),
                    ),
                    ast.FunctionDef(
                        name="__call__",
                        args=ast.arguments(
                            posonlyargs=[
                                ast.arg(arg="self"),
                                ast.arg(arg="key", annotation=ast.Name(id="str")),
                            ],
                            args=[],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=ast.arg(
                                arg="kwargs",
                                annotation=ast.Subscript(
                                    value=ast.Name(id="dict"),
                                    slice=ast.Tuple(
                                        elts=[
                                            ast.Name(id="str"),
                                            ast.Name(id="Any"),
                                        ],
                                    ),
                                ),
                            ),
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[],
                        returns=ast.Name(id="LazyProxy"),
                    ),
                ],
                decorator_list=[],
            ),
            ast.AnnAssign(target=ast.Name(id="L"), annotation=ast.Name(id="LazyFactory"), simple=1),
        ],
        type_ignores=[],
    )


def build_ast(node: Node) -> list[ast.ClassDef]:
    body: list[ast.ClassDef] = []
    cls_dict: dict[str, ClassDefWrapper] = {
        to_camel_case(node.name): ClassDefWrapper(
            name=to_camel_case(node.name),
            class_def=ast.ClassDef(
                name=to_camel_case(node.name),
                bases=[],
                keywords=[],
                body=[],
                decorator_list=[],
            ),
            attributes=[inner_node.name for inner_node in node.attributes if inner_node.attributes],
            methods={
                FunctionDefWrapper(
                    name=inner_node.name,
                    function_def=ast.FunctionDef(
                        name=inner_node.name,
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=None,
                            defaults=[],
                        ),
                        body=[],
                        decorator_list=[],
                    ),
                    args=tuple(inner_node.args),
                )
                for inner_node in node.attributes
            },
        ),
    }

    for inner_node in node.attributes:
        fill_ast(inner_node, cls_dict)

    for class_wrapper in cls_dict.values():
        if class_wrapper.attributes:
            seen = set()

            for attribute in class_wrapper.attributes:
                if attribute not in seen:
                    class_wrapper.class_def.body.append(
                        ast.Assign(
                            targets=[ast.Name(id=attribute)],
                            value=ast.Call(
                                func=ast.Name(id=to_camel_case(attribute)),
                                args=[],
                                keywords=[],
                            ),
                        ),
                    )
                    seen.add(attribute)

        if class_wrapper.methods:
            class_wrapper.class_def.body.extend(
                [
                    ast.FunctionDef(
                        name=method.name,
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[ast.arg(arg="self")],
                            vararg=None,
                            kwonlyargs=[
                                ast.arg(arg=arg, annotation=ast.Name(id="Any"))
                                for arg in method.args
                            ],
                            kw_defaults=[None] * len(method.args),
                            kwarg=None,
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[ast.Name(id="overload")],
                        returns=ast.Name(id="str"),
                    )
                    if method.args
                    else ast.FunctionDef(
                        name=method.name,
                        args=ast.arguments(
                            posonlyargs=[],
                            args=[ast.arg(arg="self")],
                            vararg=None,
                            kwonlyargs=[],
                            kw_defaults=[],
                            kwarg=None,
                            defaults=[],
                        ),
                        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
                        decorator_list=[ast.Name(id="overload")],
                        returns=ast.Name(id="str"),
                    )
                    for method in class_wrapper.methods
                ],
            )

        body.insert(0, class_wrapper.class_def)

    return body


def fill_ast(node: Node, cls_dict: dict[str, ClassDefWrapper]) -> None:
    if to_camel_case(node.name) not in cls_dict:
        if node.attributes:
            class_wrapper = ClassDefWrapper(
                name=to_camel_case(node.name),
                class_def=ast.ClassDef(
                    name=to_camel_case(node.name),
                    bases=[],
                    keywords=[],
                    body=[],
                    decorator_list=[],
                ),
                attributes=[
                    inner_node.name for inner_node in node.attributes if inner_node.attributes
                ],
                methods={
                    FunctionDefWrapper(
                        name=inner_node.name,
                        function_def=ast.FunctionDef(
                            name=inner_node.name,
                            args=ast.arguments(
                                posonlyargs=[],
                                args=[],
                                vararg=None,
                                kwonlyargs=[],
                                kw_defaults=[],
                                kwarg=None,
                                defaults=[],
                            ),
                            body=[],
                            decorator_list=[],
                        ),
                        args=tuple(inner_node.args),
                    )
                    for inner_node in node.attributes
                    if not inner_node.attributes
                },
            )
            cls_dict[class_wrapper.name] = class_wrapper

    else:
        cls_dict[to_camel_case(node.name)].methods |= {
            FunctionDefWrapper(
                name=inner_node.name,
                function_def=ast.FunctionDef(
                    name=inner_node.name,
                    args=ast.arguments(
                        posonlyargs=[],
                        args=[],
                        vararg=None,
                        kwonlyargs=[],
                        kw_defaults=[],
                        kwarg=None,
                        defaults=[],
                    ),
                    body=[],
                    decorator_list=[],
                ),
                args=tuple(inner_node.args),
            )
            for inner_node in node.attributes
            if not inner_node.attributes
        }

        cls_dict[to_camel_case(node.name)].attributes.extend(
            [inner_node.name for inner_node in node.attributes if inner_node.attributes],
        )

    for inner_node in node.attributes:
        fill_ast(inner_node, cls_dict)


def generate_stubs(ftl_path: Path, output_path: Path) -> None:
    if not ftl_path.exists():
        msg = f"{ftl_path} does not exists"
        raise FileExistsError(msg)

    ftl_files = locate_ftl_files(ftl_path)
    visitor = FluentVisitor()

    for ftl_file in ftl_files:
        read_ftl_messages(visitor, ftl_file)

    node = create_node(visitor.messages)

    tree = build_base_ast()
    body = build_ast(node)
    tree.body.extend(cast(Iterable[stmt], body))
    ast.fix_missing_locations(tree)

    if output_path.is_dir():
        output_path /= "stub.pyi"

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(ast.unparse(tree))
