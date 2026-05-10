"""Setup shim that marks the wheel as platform-specific.

The package ships a native shared library, so the produced wheel must NOT be
tagged as ``py3-none-any``; it must carry a real platform tag. We do this by
overriding ``bdist_wheel.root_is_pure``.
"""

from setuptools import setup

try:
    from setuptools.command.bdist_wheel import bdist_wheel  # type: ignore
except ImportError:  # older setuptools
    from wheel.bdist_wheel import bdist_wheel  # type: ignore


class _bdist_wheel(bdist_wheel):  # type: ignore[misc, valid-type]
    def finalize_options(self) -> None:
        super().finalize_options()
        self.root_is_pure = False


setup(cmdclass={"bdist_wheel": _bdist_wheel})
