import React, { useState, useRef, useEffect } from "react";
import { Modal, Form, Input, Select, InputNumber, Switch, Radio, Button, message } from "antd";
import { useAppStore } from "../../stores/appStore";
import { useConnectionStore } from "../../stores/connectionStore";
import { ConnectionConfig } from "../../types/connection";

interface ConnectionFormProps {
  open: boolean;
  onCancel: () => void;
  initialValues?: ConnectionConfig;
}

const ConnectionForm: React.FC<ConnectionFormProps> = ({ open, onCancel, initialValues }) => {
  const [form] = Form.useForm();
  const [connType, setConnType] = useState<string>(initialValues?.conn_type || "SSH");
  const [connecting, setConnecting] = useState(false);
  const addTab = useAppStore((s) => s.addTab);
  const closeConnectionForm = useAppStore((s) => s.closeConnectionForm);
  const setChannel = useAppStore((s) => s.setChannel);
  const addConfig = useConnectionStore((s) => s.addConfig);
  const updateConfig = useConnectionStore((s) => s.updateConfig);

  const isEdit = !!initialValues;

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

  const handleConnect = async (values: any) => {
    setConnecting(true);
    try {
      const { invoke, Channel } = await import("@tauri-apps/api/core");

      // --- Edit mode: persist the updated config (same id), no auto-connect. ---
      if (isEdit && initialValues) {
        const updated: ConnectionConfig = {
          id: initialValues.id,
          name: values.name || `${values.host || '本地Shell'}:${values.port || (values.conn_type === 'SSH' ? 22 : values.conn_type === 'Telnet' ? 23 : '')}`,
          conn_type: values.conn_type,
          host: values.host,
          port: values.port,
          username: values.username,
          auth: buildAuth(values),
          shell: values.shell,
          tags: initialValues.tags ?? [],
        };
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
      const config: ConnectionConfig = {
        id,
        name: values.name || `${values.host || '本地Shell'}:${values.port || (values.conn_type === 'SSH' ? 22 : values.conn_type === 'Telnet' ? 23 : '')}`,
        conn_type: values.conn_type,
        host: values.host,
        port: values.port,
        username: values.username,
        auth: buildAuth(values),
        shell: values.shell,
        tags: [],
      };

      // Connect based on type — MUST pass channel to Rust command
      if (values.conn_type === "SSH") {
        await invoke("connect_ssh", {
          id,
          host: values.host,
          port: values.port || 22,
          username: values.username,
          auth: config.auth,
          channel,  // <-- Tauri Channel object, key name must match Rust param name
        });
      } else if (values.conn_type === "Telnet") {
        await invoke("connect_telnet", {
          id,
          host: values.host,
          port: values.port || 23,
          channel,
        });
      } else if (values.conn_type === "LocalShell") {
        await invoke("connect_local", {
          id,
          shell: values.shell || null,
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
          <Select onChange={(v) => setConnType(v)}>
            <Select.Option value="SSH">SSH</Select.Option>
            <Select.Option value="Telnet">Telnet</Select.Option>
            <Select.Option value="LocalShell">本地 Shell</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item name="name" label="名称">
          <Input placeholder="连接名称（可选）" autoComplete="off" />
        </Form.Item>

        {connType !== "LocalShell" && (
          <>
            <Form.Item name="host" label="主机" rules={[{ required: true, message: "请输入主机地址" }]}>
              <Input placeholder="192.168.1.1 或 example.com" autoComplete="off" />
            </Form.Item>
            <Form.Item name="port" label="端口">
              <InputNumber min={1} max={65535} style={{ width: "100%" }} />
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

        {!isEdit && (
          <Form.Item name="save_config" label="保存配置" valuePropName="checked">
            <Switch />
          </Form.Item>
        )}

        <Form.Item>
          <Button type="primary" htmlType="submit" block loading={connecting}>
            {isEdit ? "保存" : "连接"}
          </Button>
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default ConnectionForm;