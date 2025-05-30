import argparse
import os
from pathlib import Path
import shutil
import subprocess
from qchatbuild import build
from const import CLI_BINARY_NAME, CLI_PACKAGE_NAME, PTY_BINARY_NAME
from doc import run_doc
from rust import cargo_cmd_name, rust_env
from test import all_tests
from util import Variant, get_variants


class StoreIfNotEmptyAction(argparse.Action):
    def __call__(self, parser, namespace, values, option_string=None):
        if values and len(values) > 0:
            setattr(namespace, self.dest, values)


parser = argparse.ArgumentParser(
    prog="build",
    description="Builds the qchat binary",
)
subparsers = parser.add_subparsers(help="sub-command help", dest="subparser", required=True)

build_subparser = subparsers.add_parser(name="build")
build_subparser.add_argument(
    "--output-bucket",
    action=StoreIfNotEmptyAction,
    help="The name of bucket to store the build artifacts",
)
build_subparser.add_argument(
    "--signing-bucket",
    action=StoreIfNotEmptyAction,
    help="The name of bucket to store the build artifacts",
)
build_subparser.add_argument(
    "--aws-account-id",
    action=StoreIfNotEmptyAction,
    help="The AWS account ID",
)
build_subparser.add_argument(
    "--aws-region",
    action=StoreIfNotEmptyAction,
    help="The AWS region",
)
build_subparser.add_argument(
    "--apple-id-secret",
    action=StoreIfNotEmptyAction,
    help="The Apple ID secret",
)
build_subparser.add_argument(
    "--signing-role-name",
    action=StoreIfNotEmptyAction,
    help="The name of the signing role",
)
build_subparser.add_argument(
    "--stage-name",
    action=StoreIfNotEmptyAction,
    help="The name of the stage",
)
build_subparser.add_argument(
    "--not-release",
    action="store_true",
    help="Build a non-release version",
)
build_subparser.add_argument(
    "--skip-tests",
    action="store_true",
    help="Skip running npm and rust tests",
)
build_subparser.add_argument(
    "--skip-lints",
    action="store_true",
    help="Skip running lints",
)

args = parser.parse_args()

match args.subparser:
    case "build":
        build(
            release=not args.not_release,
            output_bucket=args.output_bucket,
            signing_bucket=args.signing_bucket,
            aws_account_id=args.aws_account_id,
            aws_region=args.aws_region,
            apple_id_secret=args.apple_id_secret,
            signing_role_name=args.signing_role_name,
            stage_name=args.stage_name,
            run_lints=not args.skip_lints,
            run_test=not args.skip_tests,
        )
    case _:
        raise ValueError(f"Unsupported subparser {args.subparser}")
