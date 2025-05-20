import pathlib
import os
import shutil
from typing import Mapping, Sequence
from build import generate_sha
from const import CHAT_BINARY_NAME, CHAT_PACKAGE_NAME, LINUX_ARCHIVE_NAME
from signing import (
    CdSigningData,
    CdSigningType,
    load_gpg_signer,
    rebundle_dmg,
    cd_sign_file,
    apple_notarize_file,
)
from util import info, isDarwin, run_cmd
from rust import cargo_cmd_name, rust_env, rust_targets

BUILD_DIR_RELATIVE = pathlib.Path(os.environ.get("BUILD_DIR") or "build")
BUILD_DIR = BUILD_DIR_RELATIVE.absolute()


def run_cargo_tests():
    args = [cargo_cmd_name()]
    args.extend(["test", "--locked", "--package", CHAT_PACKAGE_NAME])
    run_cmd(
        args,
        env={
            **os.environ,
            **rust_env(release=False),
        },
    )


def run_clippy():
    args = [cargo_cmd_name(), "clippy", "--locked", "--package", CHAT_PACKAGE_NAME]
    run_cmd(
        args,
        env={
            **os.environ,
            **rust_env(release=False),
        },
    )


def build_chat_bin(
    release: bool,
    output_name: str | None = None,
    targets: Sequence[str] = [],
):
    package = CHAT_PACKAGE_NAME

    args = [cargo_cmd_name(), "build", "--locked", "--package", package]

    if release:
        args.append("--release")

    if release:
        target_subdir = "release"
    else:
        target_subdir = "debug"

    # create "universal" binary for macos
    if isDarwin():
        out_path = BUILD_DIR / f"{output_name or package}-universal-apple-darwin"
        args = [
            "lipo",
            "-create",
            "-output",
            out_path,
        ]
        for target in targets:
            args.append(pathlib.Path("target") / target / target_subdir / package)
        run_cmd(args)
        return out_path
    else:
        # linux does not cross compile arch
        target = targets[0]
        target_path = pathlib.Path("target") / target / target_subdir / package
        out_path = BUILD_DIR / "bin" / f"{(output_name or package)}-{target}"
        out_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(target_path, out_path)
        return out_path


def build_macos(chat_path: pathlib.Path):
    pass


def build_linux(chat_path: pathlib.Path):
    """
    Creates tar.gz, tar.xz, tar.zst, and zip archives under `BUILD_DIR`.

    Each archive has the following structure:
    - archive/qchat
    """
    archive_name = CHAT_BINARY_NAME

    archive_path = pathlib.Path(archive_name)
    archive_path.mkdir(parents=True, exist_ok=True)
    shutil.copy2(chat_path, archive_path / CHAT_BINARY_NAME)

    signer = load_gpg_signer()

    info(f"Building {archive_name}.tar.gz")
    tar_gz_path = BUILD_DIR / f"{archive_name}.tar.gz"
    run_cmd(["tar", "-czf", tar_gz_path, archive_path])
    generate_sha(tar_gz_path)
    if signer:
        signer.sign_file(tar_gz_path)

    info(f"Building {archive_name}.tar.xz")
    tar_xz_path = BUILD_DIR / f"{archive_name}.tar.xz"
    run_cmd(["tar", "-cJf", tar_xz_path, archive_path])
    generate_sha(tar_xz_path)
    if signer:
        signer.sign_file(tar_xz_path)

    info(f"Building {archive_name}.tar.zst")
    tar_zst_path = BUILD_DIR / f"{archive_name}.tar.zst"
    run_cmd(["tar", "-I", "zstd", "-cf", tar_zst_path, archive_path], {"ZSTD_CLEVEL": "19"})
    generate_sha(tar_zst_path)
    if signer:
        signer.sign_file(tar_zst_path)

    info(f"Building {archive_name}.zip")
    zip_path = BUILD_DIR / f"{archive_name}.zip"
    run_cmd(["zip", "-r", zip_path, archive_path])
    generate_sha(zip_path)
    if signer:
        signer.sign_file(zip_path)

    # clean up
    shutil.rmtree(archive_path)
    if signer:
        signer.clean()


def build(
    release: bool,
    output_bucket: str | None = None,
    signing_bucket: str | None = None,
    aws_account_id: str | None = None,
    apple_id_secret: str | None = None,
    signing_role_name: str | None = None,
    stage_name: str | None = None,
    run_lints: bool = True,
    run_test: bool = True,
):
    if signing_bucket and aws_account_id and apple_id_secret and signing_role_name:
        signing_data = CdSigningData(
            bucket_name=signing_bucket,
            aws_account_id=aws_account_id,
            notarizing_secret_id=apple_id_secret,
            signing_role_name=signing_role_name,
        )
    else:
        signing_data = None

    match stage_name:
        case "prod" | None:
            info("Building for prod")
        case "gamma":
            info("Building for gamma")
        case _:
            raise ValueError(f"Unknown stage name: {stage_name}")

    targets = rust_targets()

    info(f"Release: {release}")
    info(f"Targets: {targets}")
    info(f"Signing app: {signing_data is not None}")

    if run_test:
        info("Running cargo tests")
        run_cargo_tests()

    if run_lints:
        info("Running cargo clippy")
        run_clippy()

    info("Building", CHAT_PACKAGE_NAME)
    chat_path = build_chat_bin(
        release=release,
        output_name=CHAT_BINARY_NAME,
        targets=targets,
    )

    pass
