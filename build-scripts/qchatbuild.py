import json
import pathlib
from functools import cache
import os
import shutil
import time
from typing import Any, Mapping, Sequence
from build import generate_sha
from const import APPLE_TEAM_ID, CHAT_BINARY_NAME, CHAT_PACKAGE_NAME, LINUX_ARCHIVE_NAME
from signing import (
    CdSigningData,
    CdSigningType,
    load_gpg_signer,
    rebundle_dmg,
    cd_sign_file,
    apple_notarize_file,
)
from util import info, isDarwin, run_cmd, warn
from rust import cargo_cmd_name, rust_env, rust_targets
from importlib import import_module

BUILD_DIR_RELATIVE = pathlib.Path(os.environ.get("BUILD_DIR") or "build")
BUILD_DIR = BUILD_DIR_RELATIVE.absolute()

REGION = "us-west-2"
SIGNING_API_BASE_URL = "https://api.signer.builder-tools.aws.dev"


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

    for target in targets:
        args.extend(["--target", target])

    if release:
        args.append("--release")
        target_subdir = "release"
    else:
        target_subdir = "debug"

    run_cmd(
        args,
        env={
            **os.environ,
            **rust_env(release=release),
        },
    )

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


@cache
def get_creds():
    boto3 = import_module("boto3")
    session = boto3.Session()
    credentials = session.get_credentials()
    creds = credentials.get_frozen_credentials()
    return creds


def cd_signer_request(method: str, path: str, data: str | None = None):
    """
    Sends a request to the CD Signer API.
    """
    SigV4Auth = import_module("botocore.auth").SigV4Auth
    AWSRequest = import_module("botocore.awsrequest").AWSRequest
    requests = import_module("requests")

    url = f"{SIGNING_API_BASE_URL}{path}"
    headers = {"Content-Type": "application/json"}
    request = AWSRequest(method=method, url=url, data=data, headers=headers)
    SigV4Auth(get_creds(), "signer-builder-tools", REGION).add_auth(request)

    for i in range(1, 8):
        response = requests.request(method=method, url=url, headers=dict(request.headers), data=data)
        info(f"CDSigner Request ({url}): {response.status_code}")
        if response.status_code == 429:
            warn(f"Too many requests, backing off for {2**i} seconds")
            time.sleep(2**i)
            continue
        return response

    raise Exception(f"Failed to request {url}")


def cd_signer_create_request(manifest: Any) -> str:
    """
    Sends a POST request to create a new signing request. After creation, we
    need to send another request to start it.
    """
    response = cd_signer_request(
        method="POST",
        path="/signing_requests",
        data=json.dumps({"manifest": manifest}),
    )
    response_json = response.json()
    info(f"Signing request create: {response_json}")
    request_id = response_json["signingRequestId"]
    return request_id


def cd_signer_start_request(request_id: str, source_key: str, destination_key: str, signing_data: CdSigningData):
    """
    Sends a POST request to start the signing process.
    """
    response_text = cd_signer_request(
        method="POST",
        path=f"/signing_requests/{request_id}/start",
        data=json.dumps(
            {
                "iamRole": f"arn:aws:iam::{signing_data.aws_account_id}:role/{signing_data.signing_role_name}",
                "s3Location": {
                    "bucket": signing_data.bucket_name,
                    "sourceKey": source_key,
                    "destinationKey": destination_key,
                },
            }
        ),
    ).text
    info(f"Signing request start: {response_text}")


def cd_signer_status_request(request_id: str):
    response_json = cd_signer_request(
        method="GET",
        path=f"/signing_requests/{request_id}",
    ).json()
    info(f"Signing request status: {response_json}")
    return response_json["signingRequest"]["status"]


def cd_build_signed_package(file_path: pathlib.Path):
    """
    Creates a tarball `package.tar.gz` with the following structure:
    ```
    package
    ├─ manifest.yaml
    ├─ artifact
    | ├─ EXECUTABLES_TO_SIGN
    | | ├─ qchat
    ```
    """
    # working_dir = BUILD_DIR / "package"
    # shutil.rmtree(working_dir, ignore_errors=True)
    # (BUILD_DIR / "package" / "artifact" / "EXECUTABLES_TO_SIGN").mkdir(parents=True)
    #
    # name = file_path.name
    #
    # # Write the manifest.yaml
    # manifest_template_path = pathlib.Path.cwd() / "build-config" / "signing" / "qchat" / "manifest.yaml.template"
    # (working_dir / "manifest.yaml").write_text(manifest_template_path.read_text().replace("__NAME__", name))
    #
    # shutil.copy2(file_path, working_dir / "artifact" / "EXECUTABLES_TO_SIGN" / file_path.name)
    # file_path.unlink()
    #
    # run_cmd(
    #     ["gtar", "-czf", BUILD_DIR / "package.tar.gz", "manifest.yaml", "artifact"],
    #     cwd=working_dir,
    # )

    # Trying a different format without manifest.yaml and placing EXECUTABLES_TO_SIGN
    # at the root.
    # The docs contain conflicting information, idk what to even do here
    working_dir = BUILD_DIR / "package"
    shutil.rmtree(working_dir, ignore_errors=True)
    (BUILD_DIR / "package" / "EXECUTABLES_TO_SIGN").mkdir(parents=True)

    shutil.copy2(file_path, working_dir / "EXECUTABLES_TO_SIGN" / file_path.name)
    file_path.unlink()

    run_cmd(
        ["gtar", "-czf", BUILD_DIR / "package.tar.gz", "."],
        cwd=working_dir,
    )

    return BUILD_DIR / "package.tar.gz"


def manifest(
    name: str,
    identifier: str,
):
    """
    Creates the required manifest argument when submitting the signing task. This has the same
    structure as the manifest.yaml.template under `build-config/signing/qchat/manifest.yaml.template`
    """
    return {
        "type": "app",
        "os": "osx",
        "name": name,
        "outputs": [{"label": "macos", "path": name}],
        "app": {
            "identifier": identifier,
            "signing_requirements": {
                "certificate_type": "developerIDAppDistribution",
                "app_id_prefix": APPLE_TEAM_ID,
            },
        },
    }


def sign_executable(signing_data: CdSigningData, chat_path: pathlib.Path):
    name = chat_path.name
    info(f"Signing {name}")

    info("Packaging...")
    package_path = cd_build_signed_package(chat_path)

    info("Uploading...")
    run_cmd(["aws", "s3", "rm", "--recursive", f"s3://{signing_data.bucket_name}/signed"])
    run_cmd(["aws", "s3", "rm", "--recursive", f"s3://{signing_data.bucket_name}/pre-signed"])
    run_cmd(["aws", "s3", "cp", package_path, f"s3://{signing_data.bucket_name}/pre-signed/package.tar.gz"])

    info("Sending request...")
    request_id = cd_signer_create_request(manifest(name, "com.amazon.codewhisperer"))
    cd_signer_start_request(
        request_id=request_id,
        source_key="pre-signed/package.tar.gz",
        destination_key="signed/signed.zip",
        signing_data=signing_data,
    )

    max_duration = 180
    end_time = time.time() + max_duration
    i = 1
    while True:
        info(f"Checking for signed package attempt #{i}")
        status = cd_signer_status_request(request_id)
        info(f"Package has status: {status}")

        match status:
            case "success":
                break
            case "created" | "processing" | "inProgress":
                pass
            case "failure":
                raise RuntimeError("Signing request failed")
            case _:
                warn(f"Unexpected status, ignoring: {status}")

        if time.time() >= end_time:
            raise RuntimeError("Signed package did not appear, check signer logs")
        time.sleep(2)
        i += 1

    info("Signed!")

    info("Downloading...")
    run_cmd(["aws", "s3", "cp", f"s3://{signing_data.bucket_name}/signed/signed.zip", "signed.zip"])
    run_cmd(["unzip", "signed.zip"])


def sign_and_notarize(signing_data: CdSigningData, chat_path: pathlib.Path):
    # First, sign the application
    sign_executable(signing_data, chat_path)

    # Next, notarize the application

    # Last, staple the notarization to the application
    pass


def build_macos(chat_path: pathlib.Path, signing_data: CdSigningData | None):
    chat_dst = BUILD_DIR / "qchat"
    chat_dst.unlink(missing_ok=True)
    shutil.copy2(chat_path, chat_dst)

    if signing_data:
        sign_and_notarize(signing_data, chat_dst)

    return chat_dst


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

    if isDarwin():
        if signing_bucket and aws_account_id and apple_id_secret and signing_role_name:
            signing_data = CdSigningData(
                bucket_name=signing_bucket,
                aws_account_id=aws_account_id,
                notarizing_secret_id=apple_id_secret,
                signing_role_name=signing_role_name,
            )
        else:
            signing_data = None

        chat_path = build_macos(chat_path, signing_data)
        sha_path = generate_sha(chat_path)

        if output_bucket:
            staging_location = f"s3://{output_bucket}/staging/"
            info(f"Build complete, sending to {staging_location}")
            run_cmd(["aws", "s3", "cp", chat_path, staging_location])
            run_cmd(["aws", "s3", "cp", sha_path, staging_location])
    else:
        build_linux(chat_path)
