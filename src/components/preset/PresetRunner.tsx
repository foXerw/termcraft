import React, { useState, useEffect } from "react";
import { Modal, Form, Input, Button, Space, Progress, Typography, Descriptions } from "antd";
import { PlayCircleOutlined, PauseCircleOutlined, StopOutlined } from "@ant-design/icons";
import { Preset, PresetExecutionStatus } from "../../types/preset";
import { usePresetStore } from "../../stores/presetStore";
import { useAppStore } from "../../stores/appStore";

interface PresetRunnerProps {
  preset: Preset;
  onClose: () => void;
}

const PresetRunner: React.FC<PresetRunnerProps> = ({ preset, onClose }) => {
  const [variableValues, setVariableValues] = useState<Record<string, string>>({});
  const [execId, setExecId] = useState<string | null>(null);
  const [status, setStatus] = useState<PresetExecutionStatus | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const activeTabId = useAppStore((s) => s.activeTabId);
  const tabs = useAppStore((s) => s.tabs);

  // Initialize variable defaults
  useEffect(() => {
    const defaults: Record<string, string> = {};
    preset.variables.forEach((v) => {
      defaults[v.name] = v.default_value || "";
    });
    setVariableValues(defaults);
  }, [preset]);

  const handleStart = async () => {
    try {
      const { invoke, Channel } = await import("@tauri-apps/api/core");
      const id = crypto.randomUUID();
      setExecId(id);

      // Find active connection
      const activeTab = tabs.find((t) => t.id === activeTabId);
      if (!activeTab) {
        console.error("No active connection");
        return;
      }

      const statusChannel = new Channel<string>();
      statusChannel.onmessage = (msg) => {
        try {
          const parsed = JSON.parse(msg) as PresetExecutionStatus;
          setStatus(parsed);
          if (parsed.state === "Completed" || parsed.state === "Failed" || parsed.state === "Cancelled") {
            setIsRunning(false);
          }
        } catch (e) {
          console.error("Failed to parse status:", e);
        }
      };

      await invoke("execute_preset", {
        execId: id,
        presetId: preset.id,
        connectionId: activeTab.connectionId,
        variables: variableValues,
        statusChannel,
      });

      setIsRunning(true);
    } catch (e) {
      console.error("Execute preset failed:", e);
    }
  };

  const handleStop = async () => {
    if (!execId) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("stop_preset", { execId });
      setIsRunning(false);
    } catch (e) {
      console.error("Stop preset failed:", e);
    }
  };

  const handlePause = async () => {
    if (!execId) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("pause_preset", { execId });
    } catch (e) {
      console.error("Pause preset failed:", e);
    }
  };

  const enabledCommands = preset.commands.filter((c) => c.enabled);
  const progress = status
    ? (status.current_command_index / status.total_commands) * 100
    : 0;

  return (
    <Modal
      title={`执行预设: ${preset.name}`}
      open={true}
      onCancel={onClose}
      footer={null}
      width={480}
    >
      <Descriptions size="small" column={1} bordered>
        <Descriptions.Item label="执行模式">
          {preset.execution_mode.type === "Single" ? "单条" :
           preset.execution_mode.type === "Batch" ? "批量" : "循环"}
        </Descriptions.Item>
        <Descriptions.Item label="命令数量">
          {enabledCommands.length}
        </Descriptions.Item>
        <Descriptions.Item label="目标连接">
          {tabs.find((t) => t.id === activeTabId)?.title || "无活跃连接"}
        </Descriptions.Item>
      </Descriptions>

      {/* Variable input */}
      {preset.variables.length > 0 && !isRunning && (
        <div style={{ marginTop: 16 }}>
          <Typography.Text strong>变量值</Typography.Text>
          {preset.variables.map((v) => (
            <div key={v.name} style={{ marginTop: 8 }}>
              <Space>
                <Typography.Text>{"{{" + v.name + "}}"}</Typography.Text>
                <Input
                  placeholder={v.description || `输入 ${v.name} 的值`}
                  value={variableValues[v.name]}
                  onChange={(e) => setVariableValues({ ...variableValues, [v.name]: e.target.value })}
                />
              </Space>
            </div>
          ))}
        </div>
      )}

      {/* Execution control */}
      <div style={{ marginTop: 16, textAlign: "center" }}>
        <Space>
          {!isRunning ? (
            <Button type="primary" icon={<PlayCircleOutlined />} onClick={handleStart}
              disabled={!activeTabId || preset.variables.some((v) => !variableValues[v.name])}>
              开始执行
            </Button>
          ) : (
            <>
              <Button icon={<PauseCircleOutlined />} onClick={handlePause}>
                暂停
              </Button>
              <Button danger icon={<StopOutlined />} onClick={handleStop}>
                停止
              </Button>
            </>
          )}
        </Space>
      </div>

      {/* Progress / outcome */}
      {status && (
        <div style={{ marginTop: 16 }}>
          <Progress percent={Math.round(progress)} status={
            status.state === "Running" ? "active" :
            status.state === "Completed" ? "success" :
            status.state === "Failed" ? "exception" : "normal"
          } />
          <Space style={{ marginTop: 4, width: "100%" }}>
            {status.command_succeeded === true && (
              <Typography.Text type="success" style={{ fontSize: 12 }}>✓ 匹配</Typography.Text>
            )}
            {status.command_succeeded === false && (
              <Typography.Text type="danger" style={{ fontSize: 12 }}>✗ 未通过</Typography.Text>
            )}
            <Typography.Text style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {status.message || ""}
            </Typography.Text>
          </Space>
          {preset.execution_mode.type === "Loop" && (
            <Typography.Text style={{ fontSize: 12 }}>
              第 {status.current_loop + 1} 次循环
            </Typography.Text>
          )}
          {status.captured_snippet && (
            <Input.TextArea
              value={status.captured_snippet}
              readOnly
              autoSize={{ minRows: 2, maxRows: 6 }}
              style={{ marginTop: 8, fontFamily: "monospace", fontSize: 12 }}
            />
          )}
        </div>
      )}
    </Modal>
  );
};

export default PresetRunner;