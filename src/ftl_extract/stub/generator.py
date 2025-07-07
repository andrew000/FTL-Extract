from __future__ import annotations

import ast
import json
from typing import TYPE_CHECKING, cast

import click
from fluent.syntax import FluentParser

from ftl_extract.stub.tree import METADATA_DICT_KEY, Metadata, generate_tree
from ftl_extract.stub.utils import to_camel_case
from ftl_extract.stub.visitor import FluentVisitor, Message

if TYPE_CHECKING:
    from collections.abc import Generator
    from pathlib import Path
    from typing import Any


class NoBodyError(Exception): ...


class RootKeyIsMissingError(Exception): ...


def read_ftl_messages(visitor: FluentVisitor, path: Path) -> dict[str, Message]:
    resource = FluentParser().parse(path.read_text(encoding="utf-8"))
    if resource.body is None:
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
                    ast.alias(name="Literal"),
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


def create_static_method(name: str, metadata: Metadata) -> ast.FunctionDef:
    args = ast.arguments(
        posonlyargs=[],
        args=[],
        vararg=None,
        kwonlyargs=[
            ast.arg(arg=arg_name, annotation=ast.Name(id="Any", ctx=ast.Load()))
            for arg_name in metadata["args"]
        ],
        kw_defaults=[None for _ in metadata["args"]],
        kwarg=ast.arg(arg="kwargs", annotation=ast.Name(id="Any", ctx=ast.Load())),
        defaults=[],
    )
    return ast.FunctionDef(
        name=name,
        args=args,
        decorator_list=[ast.Name(id="staticmethod", ctx=ast.Load())],
        returns=ast.Subscript(
            value=ast.Name(id="Literal", ctx=ast.Load()),
            slice=ast.Constant(value=metadata["translation"].split("\n", maxsplit=1)[0]),
            ctx=ast.Load(),
        ),
        body=[ast.Expr(value=ast.Constant(value=Ellipsis))],
        type_comment=None,
    )


def process_tree(
    name: str,
    tree: dict[str, Any],
    parent_body: list[ast.stmt],
) -> None:
    if METADATA_DICT_KEY in tree:
        static_method = create_static_method(name, cast(Metadata, tree.pop(METADATA_DICT_KEY)))
        parent_body.append(static_method)

        if tree:
            static_method.decorator_list.append(ast.Name(id="overload", ctx=ast.Load()))

    if tree:
        parent_body.insert(
            0,
            cast(
                ast.stmt,
                ast.Assign(
                    targets=[ast.Name(id=name, ctx=ast.Store())],
                    value=ast.Name(id=f"__{to_camel_case(name)}", ctx=ast.Load()),
                ),
            ),
        )

        class_def = ast.ClassDef(
            name=f"__{to_camel_case(name)}",
            bases=[],
            keywords=[],
            body=[],
            decorator_list=[],
        )

        for key, value in tree.items():
            process_tree(key, value, class_def.body)

        parent_body.append(cast(ast.stmt, class_def))


def generate_ast(module: ast.Module, tree: dict[str, dict[str, Any]]) -> None:
    if "i18n_stub" not in tree:
        msg = "i18n_stub key is missing in the tree"
        raise RootKeyIsMissingError(msg)

    top_class = ast.ClassDef(
        name="I18nStub",
        bases=[],
        keywords=[],
        body=[],
        decorator_list=[],
    )
    for key, value in tree["i18n_stub"].items():
        if key == METADATA_DICT_KEY:
            continue

        process_tree(key, value, top_class.body)

    module.body.append(top_class)


def generate_stubs(ftl_path: Path, output_path: Path, export_tree: bool = False) -> None:
    if output_path.is_dir():
        output_path /= "stub.pyi"

    if output_path.suffix != ".pyi":
        msg = f"Output file `{output_path.name}` must have `.pyi` extension"
        raise ValueError(msg)

    ftl_files = locate_ftl_files(ftl_path)
    visitor = FluentVisitor()

    for ftl_file in ftl_files:
        read_ftl_messages(visitor, ftl_file)

    tree = generate_tree(visitor.messages)
    tree: dict[str, dict[str, Any]] = {"i18n_stub": {**tree}}
    if export_tree:
        (output_path.parent / "stub.json").write_text(
            json.dumps(tree, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        click.echo(f"Tree structure exported to {output_path.parent / 'stub.json'}")

    module = build_base_ast()
    generate_ast(module, tree)
    ast.fix_missing_locations(module)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(ast.unparse(module), encoding="utf-8")

    click.echo(f"Stub file generated at {output_path}")
