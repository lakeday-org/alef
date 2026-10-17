"""Direct official asset transport preserves pinned digest and length checks."""

import json
import unittest
from unittest.mock import Mock, patch

import tool_assets


class AssetBoundaryReachedError(RuntimeError):
    pass


def official_asset(architecture):
    values = {
        "arm64": (604090068, 15770964, "12a69fcf37bb8d5bc9150c44f9a936775b73e59ff044c2c245232f198c135637"),
        "amd64": (604090006, 17017543, "0543b0844a725814c0412d3f4bed12873ce9ed0da207b15775f63a97d734aac0"),
    }
    asset_id, size, digest = values[architecture]
    name = "task_darwin_" + architecture + ".tar.gz"
    return {
        "name": name,
        "id": asset_id,
        "size": size,
        "digest": "sha256:" + digest,
        "browser_download_url": "https://github.com/go-task/task/releases/download/v3.54.0/" + name,
    }


class TaskAssetTransport(unittest.TestCase):
    def test_both_architectures_request_pinned_official_archive_without_api(self):
        for machine, architecture in [("arm64", "arm64"), ("x86_64", "amd64")]:
            calls = []
            asset = official_asset(architecture)

            def bounded(url, maximum, *, calls=calls, asset=asset):
                calls.append((url, maximum))
                if url.startswith("https://api.github.com/"):
                    return json.dumps({"assets": [asset]}).encode()
                raise AssetBoundaryReachedError("actual asset download boundary reached")

            with (
                patch.object(tool_assets.platform, "machine", return_value=machine),
                patch.object(tool_assets, "bounded", bounded),
            ):
                try:
                    tool_assets.task_asset()
                except AssetBoundaryReachedError:
                    pass
                else:
                    raise AssertionError("actual asset request was not reached")
            assert calls == [(asset["browser_download_url"], 32 * 1024**2)], calls

    def test_corrupt_archive_digest_is_rejected_before_any_archive_parse(self):
        asset = official_asset("arm64")

        def bounded(url, _maximum):
            return (
                json.dumps({"assets": [asset]}).encode()
                if url.startswith("https://api.github.com/")
                else b"x" * asset["size"]
            )

        with (
            patch.object(tool_assets.platform, "machine", return_value="arm64"),
            patch.object(tool_assets, "bounded", bounded),
            patch.object(
                tool_assets.tarfile, "open", side_effect=AssertionError("archive parser must not run")
            ) as parser,
        ):
            try:
                tool_assets.task_asset()
            except ValueError as error:
                message = str(error)
            else:
                raise AssertionError("corrupt archive digest was accepted")
        assert message == "official Task asset digest or exact length differs"
        parser.assert_not_called()

    def test_matching_digest_does_not_allow_wrong_compressed_length(self):
        asset = official_asset("arm64")

        def bounded(url, _maximum):
            return (
                json.dumps({"assets": [asset]}).encode()
                if url.startswith("https://api.github.com/")
                else b"wrong length"
            )

        digest = Mock()
        digest.hexdigest.return_value = asset["digest"].removeprefix("sha256:")
        with (
            patch.object(tool_assets.platform, "machine", return_value="arm64"),
            patch.object(tool_assets, "bounded", bounded),
            patch.object(tool_assets.hashlib, "sha256", return_value=digest),
            patch.object(
                tool_assets.tarfile, "open", side_effect=AssertionError("archive parser must not run")
            ) as parser,
        ):
            try:
                tool_assets.task_asset()
            except ValueError as error:
                message = str(error)
            else:
                raise AssertionError("wrong compressed archive length was accepted")
        assert message == "official Task asset digest or exact length differs"
        parser.assert_not_called()
