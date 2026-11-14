// Zant components written in TSX and compiled to Rust at build time
// (`docs/tsx.md`). Run it with `cargo run -p zant-tsx-gallery`. One page
// with the components a TSX app uses most.
import { useState, useTheme } from "zant";
import type { usize } from "zant";

export default function Gallery() {
  const [name, setName] = useState("Zant");
  const [agree, setAgree] = useState(true);
  const [notify, setNotify] = useState(false);
  const [plan, setPlan] = useState<usize>(1);
  const [size, setSize] = useState<usize>(0);
  const [tab, setTab] = useState<usize>(0);
  const [progress, setProgress] = useState(40);
  const [open, setOpen] = useState(false);
  const [saved, setSaved] = useState<u32>(0);
  const [docs, setDocs] = useState(0);
  const [cardClicks, setCardClicks] = useState(0);

  const theme = useTheme();

  function save() {
    setSaved(saved + 1);
    setOpen(false);
  }

  return (
    <View flex={1}>
      <View key="page" padding={24} gap={16} overflow="scroll" height="100%" alignItems="flex-start">
        <Heading level={2}>TSX component gallery</Heading>
        <Paragraph>Every component on this page is written in TSX and compiled to native Rust.</Paragraph>
        <Row gap={16} alignItems="flex-start">
          <Card key="inputs" title="Inputs" width={420} gap={12}>
            <Input key="name" value={name} placeholder="Your name" onChange={setName} />
            <Checkbox key="agree" checked={agree} onChange={setAgree}>
              I agree to the terms
            </Checkbox>
            <Switch key="notify" checked={notify} onChange={setNotify}>
              Email notifications
            </Switch>
            <RadioGroup
              key="plan"
              options={[{ label: "Starter" }, { label: "Pro" }, { label: "Enterprise", disabled: !agree }]}
              selected={plan}
              direction="row"
              onChange={setPlan}
            />
            <Select key="select" options={["Starter", "Pro", "Enterprise"]} selected={plan} width={220} onChange={setPlan} />
            <Segmented
              key="size"
              options={[{ label: "Small" }, { label: "Medium" }, { label: "Large" }]}
              selected={size}
              onChange={setSize}
            />
          </Card>
          <Card key="display" title="Display" width={420} gap={12}>
            <Row gap={8} alignItems="center">
              <Avatar text="ZT" />
              <Tag text="Native" />
              <Tag text="TSX" />
              <Tag text={agree ? "Agreed" : "Pending"} />
            </Row>
            <Alert type="success" title={`Hello, ${name}`} description="State flows from TSX into native Zant components." />
            <Alert type={agree ? "info" : "warning"} title="Terms" description={agree ? "Thanks for agreeing." : "Please accept the terms to pick Enterprise."} />
            <Progress key="progress" percent={progress} width={360} />
            <Row gap={8} alignItems="center">
              <Button key="advance" onClick={() => setProgress(progress >= 100 ? 0 : progress + 20)}>
                Advance
              </Button>
              <Tooltip text="Opens a modal dialog">
                <Button key="open" variant="primary" onClick={() => setOpen(true)}>
                  Open dialog
                </Button>
              </Tooltip>
              <IconButton key="refresh" icon="refresh" onClick={() => setProgress(0)} />
            </Row>
            <Divider />
            <Text key="saved">
              Saved {saved} {saved === 1 ? "time" : "times"}
            </Text>
          </Card>
        </Row>
        <Card key="nesting" title="Nesting" width={856} gap={12}>
          <Text key="rich" size={15}>
            Text can hold <Text weight="bold">bold</Text>, <Text italic>italic</Text>,{" "}
            <Text color={theme.colorError} underline>
              colored
            </Text>{" "}
            spans and an inline <Link onClick={() => setDocs(docs + 1)}>link</Link>
            {docs > 0 && ` (opened ${docs}×)`}.
          </Text>
          <Row gap={8} alignItems="center">
            <Button key="save-icon" variant="primary" onClick={() => setOpen(true)}>
              <Icon name="check" /> Save
            </Button>
            <Button key="copy-icon">
              <Icon name="copy" /> Copy
            </Button>
            <Tag>label as children</Tag>
          </Row>
          <Clickable key="clickable-card" onClick={() => setCardClicks(cardClicks + 1)}>
            <View
              padding={12}
              gap={8}
              radius={8}
              borderWidth={1}
              borderColor={theme.colorBorder}
              hoverBackground={theme.colorFillTertiary}
            >
              <Text>Clickable card: clicked {cardClicks} {cardClicks === 1 ? "time" : "times"}</Text>
              <Button key="inner" size="small" onClick={() => setDocs(docs + 1)}>
                Inner button (doesn't click the card)
              </Button>
            </View>
          </Clickable>
        </Card>
        <Tabs
          key="tabs"
          items={[
            { label: "Overview", content: <Paragraph>Tabs take an array of objects whose content is an element.</Paragraph> },
            { label: "Details", content: <Text>Selected plan index: {plan}, size index: {size}</Text> },
          ]}
          active={tab}
          onChange={setTab}
        />
      </View>
      {open && (
        <Modal
          key="dialog"
          title="Save changes"
          onClose={() => setOpen(false)}
          footer={
            // `open ? … : null` instead of the bare element: the Rust codegen
            // types an element for an `Option<Node>` prop as a `Node` and
            // emits `Some(..)` only for a null-union (reported as a `zant-tsx`
            // bug; `open` is always true in this `open && (…Modal)` branch, so
            // the tree is the same).
            open ? (
              <Row gap={8} justifyContent="flex-end">
                <Button key="cancel" onClick={() => setOpen(false)}>
                  Cancel
                </Button>
                <Button key="ok" variant="primary" onClick={save}>
                  Save
                </Button>
              </Row>
            ) : null
          }
        >
          <Paragraph>This dialog is a TSX element passed to the native modal.</Paragraph>
        </Modal>
      )}
    </View>
  );
}
