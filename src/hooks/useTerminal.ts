import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";

export function useTerminal(
  containerRef: React.RefObject<HTMLDivElement>,
  options: {
    fontSize?: number;
    fontFamily?: string;
    cols?: number;
    rows?: number;
    scrollback?: number;
    cursorStyle?: string;
    theme?: string;
  } = {}
) {
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);

  const initTerminal = useCallback(() => {
    if (!containerRef.current || terminalRef.current) return;

    const terminal = new Terminal({
      fontSize: options.fontSize || 14,
      fontFamily: options.fontFamily || "Consolas, 'Courier New', monospace",
      cols: options.cols || 80,
      rows: options.rows || 24,
      scrollback: options.scrollback || 5000,
      cursorStyle: (options.cursorStyle || "block") as any,
      theme: options.theme === "dark" ? {
        background: "#1e1e1e",
        foreground: "#cccccc",
        cursor: "#ffffff",
        selectionBackground: "#264f78",
      } : undefined,
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(containerRef.current);
    fitAddon.fit();

    // Try WebGL renderer
    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => webglAddon.dispose());
      terminal.loadAddon(webglAddon);
    } catch {
      // Fall back to default canvas renderer
    }

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;
  }, [options.fontSize, options.fontFamily, options.scrollback, options.theme]);

  const fitTerminal = useCallback(() => {
    if (fitAddonRef.current) {
      fitAddonRef.current.fit();
    }
  }, []);

  const writeData = useCallback((data: string) => {
    if (terminalRef.current) {
      terminalRef.current.write(data);
    }
  }, []);

  const resizeTerminal = useCallback((cols: number, rows: number) => {
    if (terminalRef.current) {
      terminalRef.current.resize(cols, rows);
    }
  }, []);

  const disposeTerminal = useCallback(() => {
    if (terminalRef.current) {
      terminalRef.current.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    }
  }, []);

  return {
    terminal: terminalRef,
    initTerminal,
    fitTerminal,
    writeData,
    resizeTerminal,
    disposeTerminal,
  };
}