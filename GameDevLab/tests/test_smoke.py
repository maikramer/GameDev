def test_import_cli():
    from gamedev_lab.cli import main

    assert callable(main)


def test_version():
    from gamedev_lab import __version__

    assert __version__
