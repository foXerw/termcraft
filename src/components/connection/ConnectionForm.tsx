import React, { useState, useRef, useEffect } from "react";
import { Modal, Form, Input, Select, InputNumber, Switch, Radio, Button, Space, AutoComplete, message } from "antd";
import { ReloadOutlined } from "@ant-design/icons";
import { useAppStore } from "../../stores/appStore";
import { useConnectionStore } from "../../stores/connectionStore";
import { ConnectionConfig, DEFAULT_SERIAL_CONFIG, SerialConfig } from "../../types/connection";

interface ConnectionFormProps {
  open: boolean;
  onCancel: () => void;
  initialValues?: ConnectionConfig;
}

const ConnectionForm: React.FC<ConnectionFormProps> = ({ open, onCancel, initialValues }) => {
  const [form] = Form.useForm();
  const [connType, setConnType] = useState<string>(initialValues?.conn_type || "SSH");
  const [connecting, setConnecting] = useState(false);
  const [serialPorts, setSerialPorts] = useState<string[]>([]);
  const [portsLoading, setPortsLoading] = useState(false);
  const addTab = useAppStore((s) => s.addTab);
  const closeConnectionForm = useAppStore((s) => s.closeConnectionForm);
  const setChannel = useAppStore((s) => s.setChannel);
  const addConfig = useConnectionStore((s) => s.addConfig);
  const updateConfig = useConnectionStore((s) => s.updateConfig);

  const isEdit = !!initialValues;

  // When switching connection type, seed the port field with that type's
  // default (SSH=22, Telnet=23) — but only if the user hasn't entered a
  // custom port. Serial/LocalShell have no port, so clear it.
  const onConnTypeChange = (v: string) => {
    setConnType(v);
    const cur = form.getFieldValue("port");
    const isDefault = cur === undefined || cur === null || cur === "" || cur === 22 || cur === 23;
    if (!isDefault) return;
    if (v === "SSH") form.setFieldValue("port", 22);
    else if (v === "Telnet") form.setFieldValue("port", 23);
    else form.setFieldValue("port", undefined);
  };

  // Sync the `connType` rendering state whenever the dialog opens. The form
  // is always mounted (only `open` toggles), so the useState initializer
  // alone can't track which connection type this open is for — without this,
  // a stale type from a previous open (e.g. Telnet) would render the wrong
  // field set even though the Select shows SSH. Also fixes edit mode, which
  // previously always rendered SSH fields regardless of the config's type.
  useEffect(() => {
    if (!open) return;
    setConnType(initialValues?.conn_type ?? "SSH");
  }, [open, initialValues]);

  // Enumerate available serial ports. Called once when the user picks Serial
  // and again on manual refresh.
  const refreshSerialPorts = async () => {
    setPortsLoading(true);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const ports = await invoke<string[]>("list_serial_ports");
      setSerialPorts(ports);
    } catch (e) {
      console.error("list_serial_ports failed:", e);
    } finally {
      setPortsLoading(false);
    }
  };

  useEffect(() => {
    if (connType === "Serial" && serialPorts.length === 0) {
      refreshSerialPorts();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connType]);

  // Map a saved ConnectionConfig onto this form's flat field names. The form
  // uses auth_type + separate password/key_path/passphrase fields, while the
  // persisted config nests them under `auth`.
  const buildInitialValues = (config?: ConnectionConfig) => {
    const authType =
      config?.auth?.type === "PublicKey" ? "key"
        : config?.auth?.type === "Agent" ? "agent"
        : "password";
    return {
      conn_type: config?.conn_type ?? "SSH",
      name: config?.name,
      host: config?.host,
      port: config?.port ?? (config?.conn_type === "Telnet" ? 23 : 22),
      username: config?.username,
      shell: config?.shell,
      // Serial fields flattened out of the nested `serial` object for the form.
      serial_port: config?.serial?.port_path,
      baud_rate: config?.serial?.baud_rate ?? DEFAULT_SERIAL_CONFIG.baud_rate,
      data_bits: config?.serial?.data_bits ?? DEFAULT_SERIAL_CONFIG.data_bits,
      parity: config?.serial?.parity ?? DEFAULT_SERIAL_CONFIG.parity,
      stop_bits: config?.serial?.stop_bits ?? DEFAULT_SERIAL_CONFIG.stop_bits,
      flow_control: config?.serial?.flow_control ?? DEFAULT_SERIAL_CONFIG.flow_control,
      auth_type: authType,
      password: config?.auth?.type === "Password" ? config.auth.password : "",
      key_path: config?.auth?.type === "PublicKey" ? config.auth.key_path : "",
      passphrase: config?.auth?.type === "PublicKey" ? config.auth.passphrase : "",
      save_config: true,
    };
  };

  // Build the nested AuthConfig from flat form values.
  const buildAuth = (values: any): ConnectionConfig["auth"] => {
    if (values.auth_type === "password") {
      return { type: "Password", password: values.password };
    }
    if (values.auth_type === "key") {
      return { type: "PublicKey", key_path: values.key_path, passphrase: values.passphrase };
    }
    return { type: "Agent" };
  };

  // Build a ConnectionConfig from flat form values. Reuses existing id/tags
  // when editing, otherwise generates a fresh id.
  const buildConfig = (values: any, id?: string): ConnectionConfig => {
    const connType = values.conn_type as ConnectionConfig["conn_type"];
    const serial: SerialConfig | undefined = connType === "Serial"
      ? {
          port_path: values.serial_port,
          baud_rate: values.baud_rate != null && values.baud_rate !== "" ? Number(values.baud_rate) : DEFAULT_SERIAL_CONFIG.baud_rate,
          data_bits: values.data_bits ?? DEFAULT_SERIAL_CONFIG.data_bits,
          parity: values.parity ?? DEFAULT_SERIAL_CONFIG.parity,
          stop_bits: values.stop_bits ?? DEFAULT_SERIAL_CONFIG.stop_bits,
          flow_control: values.flow_control ?? DEFAULT_SERIAL_CONFIG.flow_control,
        }
      : undefined;

    const defaultName =
      connType === "Serial"
        ? values.serial_port || "串口"
        : `${values.host || '本地Shell'}:${values.port || (connType === 'SSH' ? 22 : connType === 'Telnet' ? 23 : '')}`;

    return {
      id: id ?? crypto.randomUUID(),
      name: values.name || defaultName,
      conn_type: connType,
      host: values.host,
      port: values.port,
      username: values.username,
      // Serial/Telnet/LocalShell have no auth; only SSH carries credentials.
      auth: connType === "SSH" ? buildAuth(values) : undefined,
      shell: values.shell,
      serial,
      tags: initialValues?.tags ?? [],
    };
  };

  // Save without connecting — used to register a connection just for
  // reachability monitoring (e.g. testing a server you don't want to log into).
  const handleSaveOnly = async () => {
    setConnecting(true);
    try {
      const values = await form.validateFields();
      const config = buildConfig(values);
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_connection_config", { config });
      addConfig(config);
      message.success("已保存");
      form.resetFields();
      closeConnectionForm();
    } catch (e: any) {
      // validateFields rejection returns errorFields without .message
      if (e?.errorFields) return;
      console.error("Save failed:", e);
      message.error(`保存失败: ${e}`);
    } finally {
      setConnecting(false);
    }
  };

  const handleConnect = async (values: any) => {
    setConnecting(true);
    try {
      const { invoke, Channel } = await import("@tauri-apps/api/core");

      // --- Edit mode: persist the updated config (same id), no auto-connect. ---
      if (isEdit && initialValues) {
        const updated = buildConfig(values, initialValues.id);
        await invoke("save_connection_config", { config: updated });
        updateConfig(updated);
        message.success("连接已更新");
        form.resetFields();
        closeConnectionForm();
        return;
      }

      // --- New mode: connect now, optionally save. ---
      const id = crypto.randomUUID();

      // Create a Tauri Channel for this connection — Rust will stream output through it
      // We store it in appStore so TerminalView can bind its onmessage to xterm.write
      const channel = new Channel();
      setChannel(id, channel);

      // Build the connection config
      const config = buildConfig(values, id);

      // Connect based on type — MUST pass channel to Rust command
      if (values.conn_type === "SSH") {
        await invoke("connect_ssh", {
          id,
          name: config.name,
          host: values.host,
          port: values.port || 22,
          username: values.username,
          auth: config.auth,
          channel,  // <-- Tauri Channel object, key name must match Rust param name
        });
      } else if (values.conn_type === "Telnet") {
        await invoke("connect_telnet", {
          id,
          name: config.name,
          host: values.host,
          port: values.port || 23,
          channel,
        });
      } else if (values.conn_type === "LocalShell") {
        await invoke("connect_local", {
          id,
          name: config.name,
          shell: values.shell || null,
          channel,
        });
      } else if (values.conn_type === "Serial") {
        await invoke("connect_serial", {
          id,
          name: config.name,
          config: config.serial,
          channel,
        });
      }

      // Add a terminal tab
      addTab({
        id,
        connectionId: id,
        title: config.name,
        connType: values.conn_type,
        alive: true,
      });

      // Optionally save the config
      if (values.save_config) {
        await invoke("save_connection_config", { config });
        addConfig(config);
      }

      message.success("连接成功！");
      form.resetFields();
      closeConnectionForm();
    } catch (e: any) {
      console.error("Connection failed:", e);
      message.error(`连接失败: ${e}`);
    } finally {
      setConnecting(false);
    }
  };

  const handleCancel = () => {
    form.resetFields();
    onCancel();
  };

  return (
    <Modal
      title={isEdit ? "编辑连接" : "新建连接"}
      open={open}
      onCancel={handleCancel}
      footer={null}
      width={480}
      destroyOnClose
    >
      <Form
        form={form}
        layout="vertical"
        autoComplete="off"
        initialValues={buildInitialValues(initialValues)}
        onFinish={handleConnect}
      >
        <Form.Item name="conn_type" label="连接类型">
          <Select onChange={onConnTypeChange}>
            <Select.Option value="SSH">SSH</Select.Option>
            <Select.Option value="Telnet">Telnet</Select.Option>
            <Select.Option value="LocalShell">本地 Shell</Select.Option>
            <Select.Option value="Serial">串口</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item name="name" label="名称">
          <Input placeholder="连接名称（可选）" autoComplete="off" />
        </Form.Item>

        {connType !== "LocalShell" && connType !== "Serial" && (
          <>
            <Form.Item name="host" label="主机" rules={[{ required: true, message: "请输入主机地址" }]}>
              <Input placeholder="192.168.1.1 或 example.com" autoComplete="off" />
            </Form.Item>
            <Form.Item name="port" label="端口">
              <InputNumber min={1} max={65535} autoComplete="off" style={{ width: "100%" }} />
            </Form.Item>
          </>
        )}

        {connType === "SSH" && (
          <>
            <Form.Item name="username" label="用户名" rules={[{ required: true }]}>
              <Input placeholder="root" autoComplete="off" />
            </Form.Item>
            <Form.Item name="auth_type" label="认证方式">
              <Radio.Group>
                <Radio value="password">密码</Radio>
                <Radio value="key">密钥</Radio>
                <Radio value="agent">SSH Agent</Radio>
              </Radio.Group>
            </Form.Item>
            <Form.Item noStyle shouldUpdate={(prev, cur) => prev.auth_type !== cur.auth_type}>
              {({ getFieldValue }) => {
                const authType = getFieldValue("auth_type");
                if (authType === "password") {
                  return (
                    <Form.Item name="password" label="密码" rules={[{ required: true }]}>
                      <Input.Password autoComplete="new-password" />
                    </Form.Item>
                  );
                } else if (authType === "key") {
                  return (
                    <>
                      <Form.Item name="key_path" label="密钥路径" rules={[{ required: true }]}>
                        <Input placeholder="/path/to/id_rsa" autoComplete="off" />
                      </Form.Item>
                      <Form.Item name="passphrase" label="密钥密码">
                        <Input.Password placeholder="可选" autoComplete="new-password" />
                      </Form.Item>
                    </>
                  );
                }
                return null;
              }}
            </Form.Item>
          </>
        )}

        {connType === "LocalShell" && (
          <Form.Item name="shell" label="Shell 程序">
            <Input placeholder="留空使用默认 Shell（Windows: cmd.exe, Linux: bash）" autoComplete="off" />
          </Form.Item>
        )}

        {connType === "Serial" && (
          <>
            <Form.Item label="串口">
              <Space.Compact style={{ width: "100%" }}>
                <Form.Item name="serial_port" noStyle rules={[{ required: true, message: "请选择串口" }]}>
                  <Select
                    placeholder="选择串口"
                    loading={portsLoading}
                    options={serialPorts.map((p) => ({ label: p, value: p }))}
                    notFoundContent={portsLoading ? "枚举中…" : "未发现串口"}
                  />
                </Form.Item>
                <Button icon={<ReloadOutlined />} onClick={refreshSerialPorts} loading={portsLoading} />
              </Space.Compact>
            </Form.Item>
            <Form.Item
              name="baud_rate"
              label="波特率"
              rules={[
                { required: true, message: "请输入波特率" },
                {
                  validator: (_, v) => {
                    const n = Number(v);
                    if (!Number.isInteger(n) || n <= 0) {
                      return Promise.reject(new Error("波特率必须为正整数"));
                    }
                    return Promise.resolve();
                  },
                },
              ]}
            >
              <AutoComplete
                options={[300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600].map((b) => ({ value: b, label: String(b) }))}
                filterOption={(input, option) => String(option?.value ?? "").includes(input)}
                placeholder="选择或输入波特率"
              />
            </Form.Item>
            <Form.Item name="data_bits" label="数据位">
              <Select
                options={[
                  { label: "5", value: "Five" },
                  { label: "6", value: "Six" },
                  { label: "7", value: "Seven" },
                  { label: "8", value: "Eight" },
                ]}
              />
            </Form.Item>
            <Form.Item name="parity" label="校验">
              <Select
                options={[
                  { label: "None", value: "None" },
                  { label: "Odd", value: "Odd" },
                  { label: "Even", value: "Even" },
                ]}
              />
            </Form.Item>
            <Form.Item name="stop_bits" label="停止位">
              <Select
                options={[
                  { label: "1", value: "One" },
                  { label: "2", value: "Two" },
                ]}
              />
            </Form.Item>
            <Form.Item name="flow_control" label="流控">
              <Select
                options={[
                  { label: "None", value: "None" },
                  { label: "Software (XON/XOFF)", value: "Software" },
                ]}
              />
            </Form.Item>
          </>
        )}

        {!isEdit && (
          <Form.Item name="save_config" label="保存配置" valuePropName="checked">
            <Switch />
          </Form.Item>
        )}

        <Form.Item>
          <Space style={{ width: "100%", justifyContent: "flex-end" }}>
            {/* New mode: extra "save without connecting" action for registering
                a host just to monitor its reachability. */}
            {!isEdit && (
              <Button onClick={handleSaveOnly} loading={connecting}>仅保存</Button>
            )}
            <Button type="primary" htmlType="submit" loading={connecting}>
              {isEdit ? "保存" : "连接"}
            </Button>
          </Space>
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default ConnectionForm;