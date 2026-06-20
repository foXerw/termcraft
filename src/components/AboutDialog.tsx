import React from "react";
import { Modal, Typography, Button } from "antd";
import ReactMarkdown from "react-markdown";
import aboutMd from "../content/about.md?raw";
import { useAppStore } from "../stores/appStore";

const AboutDialog: React.FC = () => {
  const aboutOpen = useAppStore((s) => s.aboutOpen);
  const closeAbout = useAppStore((s) => s.closeAbout);

  return (
    <Modal
      title="关于 TermCraft"
      open={aboutOpen}
      onCancel={closeAbout}
      width={640}
      footer={[<Button key="close" onClick={closeAbout}>关闭</Button>]}
    >
      <Typography>
        <div className="about-markdown">
          <ReactMarkdown>{aboutMd}</ReactMarkdown>
        </div>
      </Typography>
    </Modal>
  );
};

export default AboutDialog;
