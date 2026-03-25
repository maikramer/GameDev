"""Testes para gamedev_shared.logging."""

from io import StringIO
from unittest.mock import patch

from gamedev_shared.logging import Logger


class TestLoggerAnsi:
    """Testes com fallback ANSI (sem Rich)."""

    def _make_logger_no_rich(self):
        logger = Logger.__new__(Logger)
        logger._console = None
        return logger

    def test_info(self, capsys):
        logger = self._make_logger_no_rich()
        logger.info("teste info")
        captured = capsys.readouterr()
        assert "[INFO]" in captured.out
        assert "teste info" in captured.out

    def test_warn(self, capsys):
        logger = self._make_logger_no_rich()
        logger.warn("aviso")
        captured = capsys.readouterr()
        assert "[WARN]" in captured.out
        assert "aviso" in captured.out

    def test_error(self, capsys):
        logger = self._make_logger_no_rich()
        logger.error("falha")
        captured = capsys.readouterr()
        assert "[ERROR]" in captured.out
        assert "falha" in captured.out

    def test_step(self, capsys):
        logger = self._make_logger_no_rich()
        logger.step("passo 1")
        captured = capsys.readouterr()
        assert "[STEP]" in captured.out
        assert "passo 1" in captured.out

    def test_success(self, capsys):
        logger = self._make_logger_no_rich()
        logger.success("feito")
        captured = capsys.readouterr()
        assert "feito" in captured.out

    def test_header(self, capsys):
        logger = self._make_logger_no_rich()
        logger.header("Secção")
        captured = capsys.readouterr()
        assert "Secção" in captured.out

    def test_panel_ansi(self, capsys):
        logger = self._make_logger_no_rich()
        logger.panel("conteúdo", title="Título")
        captured = capsys.readouterr()
        assert "Título" in captured.out
        assert "conteúdo" in captured.out

    def test_table_ansi(self, capsys):
        logger = self._make_logger_no_rich()
        logger.table([("chave", "valor")], title="Info")
        captured = capsys.readouterr()
        assert "chave" in captured.out
        assert "valor" in captured.out


class TestLoggerRich:
    """Testes com Rich (se disponível)."""

    def test_rich_available(self):
        logger = Logger()
        assert isinstance(logger.rich_available, bool)

    def test_console_property(self):
        logger = Logger()
        if logger.rich_available:
            assert logger.console is not None

    def test_info_rich(self):
        logger = Logger()
        if logger.rich_available:
            logger.info("teste rich")
