import React, { useState } from "react";
import { Modal, Form, Input, Select, InputNumber, Switch, Button, Space, Card, Tag, Popconfirm, Typography } from "antd";
import { PlusOutlined, DeleteOutlined } from "@ant-design/icons";
import { Preset, CommandItem, ExecutionMode, Variable } from "../../types/preset";
import { usePresetStore } from "../../stores/presetStore";

interface PresetEditorProps {
  preset?: Preset | null;
  onClose: () => void;
}

const PresetEditor: React.FC<PresetEditorProps> = ({ preset, onClose }) => {
  const [form] = Form.useForm();
  const [commands, setCommands] = useState<CommandItem[]>(
    preset?.commands || []
  );
  const [variables, setVariables] = useState<Variable[]>(
    preset?.variables || []
  );
  const [executionMode, setExecutionMode] = useState<ExecutionMode>(
    preset?.execution_mode || { type: "Batch", stop_on_error: false }
  );
  const [selectedGroup] = useState<string | null>(preset?.group_id || null);

  const addCommand = () => {
    setCommands([
      ...commands,
      {
        id: crypto.randomUUID(),
        command: "",
        delay_ms: 0,
        enabled: true,
      },
    ]);
  };

  const removeCommand = (index: number) => {
    setCommands(commands.filter((_, i) => i !== index));
  };

  const updateCommand = (index: number, field: string, value: any) => {
    setCommands(
      commands.map((cmd, i) =>
        i === index ? { ...cmd, [field]: value } : cmd
      )
    );
  };

  const addVariable = () => {
    setVariables([
      ...variables,
      { name: "", default_value: undefined, description: undefined },
    ]);
  };

  const removeVariable = (index: number) => {
    setVariables(variables.filter((_, i) => i !== index));
  };

  const handleSave = async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const formValues = await form.validateFields();

      const newPreset: Preset = {
        id: preset?.id || crypto.randomUUID(),
        name: formValues.name,
        group_id: selectedGroup || undefined,
        description: formValues.description || undefined,
        commands,
        variables,
        execution_mode: executionMode,
        created_at: preset?.created_at || new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };

      await invoke("save_preset", { preset: newPreset });

      if (preset) {
        usePresetStore.getState().updatePreset(newPreset);
      } else {
        usePresetStore.getState().addPreset(newPreset);
      }

      onClose();
    } catch (e) {
      console.error("Save preset failed:", e);
    }
  };

  return (
    <Modal
      title={preset ? "编辑预设" : "新建预设"}
      open={true}
      onCancel={onClose}
      onOk={handleSave}
      width={640}
      okText="保存"
    >
      <Form form={form} layout="vertical" initialValues={{ name: preset?.name, description: preset?.description }}>
        <Form.Item name="name" label="预设名称" rules={[{ required: true, message: "请输入预设名称" }]}>
          <Input placeholder="例如：部署脚本、健康检查" />
        </Form.Item>

        <Form.Item name="description" label="描述">
          <Input.TextArea placeholder="预设用途说明" rows={2} />
        </Form.Item>

        {/* Execution mode */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontWeight: 600, marginBottom: 8 }}>执行模式</div>
          <Space>
            <Select value={executionMode.type} onChange={(v) => {
              if (v === "Single") setExecutionMode({ type: "Single" });
              else if (v === "Batch") setExecutionMode({ type: "Batch", stop_on_error: false });
              else setExecutionMode({ type: "Loop", count: undefined, interval_ms: 1000, stop_on_error: false });
            }}>
              <Select.Option value="Single">单条执行</Select.Option>
              <Select.Option value="Batch">批量执行</Select.Option>
              <Select.Option value="Loop">循环执行</Select.Option>
            </Select>
            {executionMode.type === "Batch" && (
              <>
                <Switch checked={(executionMode as any).stop_on_error} onChange={(v) => setExecutionMode({ type: "Batch", stop_on_error: v })} />
                <span>出错时停止</span>
              </>
            )}
            {executionMode.type === "Loop" && (
              <>
                <InputNumber placeholder="循环次数（空=无限）" min={1} onChange={(v) => setExecutionMode({ ...executionMode, count: v ?? undefined })} />
                <span>间隔(ms)</span>
                <InputNumber value={(executionMode as any).interval_ms} min={0} onChange={(v) => setExecutionMode({ ...executionMode, interval_ms: v })} />
                <Switch checked={(executionMode as any).stop_on_error} onChange={(v) => setExecutionMode({ ...executionMode, stop_on_error: v })} />
                <span>出错时停止</span>
              </>
            )}
          </Space>
        </div>

        {/* Variables */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontWeight: 600, marginBottom: 8 }}>变量定义</div>
          <Button type="dashed" icon={<PlusOutlined />} size="small" onClick={addVariable}>
            添加变量
          </Button>
          {variables.map((v, i) => (
            <Card size="small" key={i} style={{ marginTop: 4 }}>
              <Space>
                <Input placeholder="变量名 (如 ip)" value={v.name} onChange={(e) => setVariables(variables.map((vv, ii) => ii === i ? { ...vv, name: e.target.value } : vv))} />
                <Input placeholder="默认值" value={v.default_value || ""} onChange={(e) => setVariables(variables.map((vv, ii) => ii === i ? { ...vv, default_value: e.target.value } : vv))} />
                <Input placeholder="说明" value={v.description || ""} onChange={(e) => setVariables(variables.map((vv, ii) => ii === i ? { ...vv, description: e.target.value } : vv))} />
                <Popconfirm title="删除此变量?" onConfirm={() => removeVariable(i)}>
                  <Button type="text" icon={<DeleteOutlined />} size="small" danger />
                </Popconfirm>
              </Space>
            </Card>
          ))}
        </div>

        {/* Commands list */}
        <div>
          <div style={{ fontWeight: 600, marginBottom: 8 }}>命令列表</div>
          <Button type="dashed" icon={<PlusOutlined />} size="small" onClick={addCommand}>
            添加命令
          </Button>
          {commands.map((cmd, i) => (
            <Card size="small" key={cmd.id} style={{ marginTop: 8 }}>
              <Space direction="vertical" style={{ width: "100%" }}>
                <Space>
                  <Switch checked={cmd.enabled} onChange={(v) => updateCommand(i, "enabled", v)} />
                  <Typography.Text style={{ fontSize: 12 }}>#{i + 1}</Typography.Text>
                </Space>
                <Input
                  placeholder="命令内容 (支持 {{变量名}} 替换)"
                  value={cmd.command}
                  onChange={(e) => updateCommand(i, "command", e.target.value)}
                />
                <Space>
                  <span style={{ fontSize: 12 }}>延迟(ms):</span>
                  <InputNumber value={cmd.delay_ms} min={0} onChange={(v) => updateCommand(i, "delay_ms", v || 0)} size="small" />
                  {cmd.wait_for ? (
                    <Tag color="purple" style={{ fontSize: 10 }}>⏳ 等待: {cmd.wait_for.pattern}</Tag>
                  ) : (
                    <Button type="link" size="small" onClick={() => updateCommand(i, "wait_for", { pattern: "", timeout_ms: 5000, match_type: "Contains" })}>
                      添加条件等待
                    </Button>
                  )}
                  {cmd.wait_for && (
                    <Popconfirm title="移除条件等待?" onConfirm={() => updateCommand(i, "wait_for", null)}>
                      <Button type="text" size="small" danger>移除等待</Button>
                    </Popconfirm>
                  )}
                  <Popconfirm title="删除此命令?" onConfirm={() => removeCommand(i)}>
                    <Button type="text" icon={<DeleteOutlined />} size="small" danger />
                  </Popconfirm>
                </Space>
                {cmd.wait_for && (
                  <Card size="small" type="inner" style={{ background: "var(--bg-tertiary)" }}>
                    <Space>
                      <Input placeholder="等待匹配模式" value={cmd.wait_for.pattern}
                        onChange={(e) => updateCommand(i, "wait_for", { ...cmd.wait_for!, pattern: e.target.value })} />
                      <Select value={cmd.wait_for.match_type} onChange={(v) => updateCommand(i, "wait_for", { ...cmd.wait_for!, match_type: v })} size="small">
                        <Select.Option value="Exact">精确匹配</Select.Option>
                        <Select.Option value="Contains">包含匹配</Select.Option>
                        <Select.Option value="Regex">正则匹配</Select.Option>
                      </Select>
                      <InputNumber value={cmd.wait_for.timeout_ms} min={0} onChange={(v) => updateCommand(i, "wait_for", { ...cmd.wait_for!, timeout_ms: v || 5000 })} size="small" />
                      <span style={{ fontSize: 12 }}>超时(ms)</span>
                    </Space>
                  </Card>
                )}
              </Space>
            </Card>
          ))}
        </div>
      </Form>
    </Modal>
  );
};

export default PresetEditor;