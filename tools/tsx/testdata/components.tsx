import { useState, Heading, Title, Paragraph, RadioGroup, Tooltip, ShortcutList } from "zant";

export default function Components() {
  const [index, setIndex] = useState<usize>(0);
  const [narrow, setNarrow] = useState<u8>(0);
  const [real, setReal] = useState(1.5);
  const [enabled, setEnabled] = useState(true);
  const [label, setLabel] = useState("Choice");
  const content = <Text>Reusable content</Text>;
  const optional = enabled ? <Text>Optional menu content</Text> : null;
  const integer: u32 = real as u32;
  const byte: u8 = index as u8;
  const wider: u32 = byte as u32;
  const small: f32 = real as f32;
  const large: f64 = small as f64;
  const fromInteger: f32 = index as f32;
  const asserted = integer!;
  const same = index as usize;
  const literal = 1.8 as u32;
  const text = label as string;

  function action() { setEnabled(previous => !previous); }
  function select(index: usize) { setIndex(index); }
  function log(value: string) { console.log(value); }

  return (
    <View key="components" padding={16} gap={12}>
      <Heading level={2}>Generated components: {label}</Heading>
      <Title level={1} weight="extraBold">All builders</Title>
      <Paragraph size={14} lineHeight={20}>Text content uses the normal template rules.</Paragraph>
      <Text>{integer} {byte} {wider}</Text>
      <Text>{small} {large} {fromInteger}</Text>
      <Text>{asserted} {same} {literal} {text}</Text>
      <Link text="Open" onClick={action} />
      <Input value={label} onChange={setLabel} onSubmit={action} />
      <Checkbox checked={enabled} onChange={setEnabled}>Enabled</Checkbox>
      <Switch checked={enabled} onChange={setEnabled}>Switch</Switch>
      <RadioGroup options={[{ label: "One" }, { label, disabled: !enabled }]}
        selected={index} direction="row" onChange={select} />
      <Tabs items={[
        { label: "Main", content },
        { label: "Extra", content: enabled ? <Text>A</Text> : <Text>B</Text> }
      ]} active={index} onChange={setIndex} />
      <Select options={["A", label, `Choice ${index}`]} selected={index} onChange={setNarrow} />
      <Text>Narrow index: {narrow}</Text>
      <Tooltip text="Save"><Button onClick={action}>Save</Button></Tooltip>
      <Tooltip text="Explicit child" child={content} />
      <Image src="sample.png" width={96} height={96} fit="contain" radius={8} />
      <Space direction="row" gap={8}>
        <Icon name="chevron_down" />
        <Icon name="chevron-down" />
        <Icon name="chevronDown" />
        <IconButton icon="chevron_down" onClick={action} />
        <IconButton icon="chevron-down" onClick={action} />
        <IconButton icon="chevronDown" onClick={action} />
      </Space>
      <DismissLayer onDismiss={action} background="#00000000" />
      <RichText width={240} lineHeight={20} onLink={log} />
      <PdfPreview src="sample.pdf" page={0} width={240} height={160}
        showToolbar={false} onPageChange={setIndex} />
      <Divider margin={{ top: 4, bottom: 4 }} color="#ddeeff" />
      <Tag text={enabled! ? "Active" : "Paused"} />
      <Card title="Explicit children" gap={8} children={[
        <Text>Array child</Text>, enabled && <Text>Conditional array child</Text>
      ]} />
      <Alert type="errorState" title="Notice" description="All reflected props are checked." />
      <Avatar text="XY" size={32} />
      <Progress percent={real} width={200} />
      <DropdownMenu items={[
        { key: "save", label, icon: "chevronDown", onClick: () => setEnabled(!enabled) },
        { label: "Indexed", indexAction: setIndex, index: 2, content: optional },
        { label: "Empty content", content: null }
      ]} width={240} motion="slideUp" motionOrigin="topLeft" />
      <Segmented options={[{ label: "A", icon: "eyeOff" }, { label: "B", tooltip: "Second" }]}
        selected={index} onChange={setReal} />
      <Modal title="Child content" footer={enabled ? <Button onClick={action}>OK</Button> : null}
        onClose={action}><Text>Dialog body</Text></Modal>
      <Modal title="Explicit content" content={content} footer={null} onClose={action} />
      <ShortcutList groups={[
        { title: "Editing", items: [{ label: "Save", keys: "mod+s" }, { label, keys: "mod+shift+s" }] }
      ]} columns={2} />
      <Message text="Generated tags" kind="success" />
      <HotkeyInput recording={enabled} placeholder="Record shortcut"
        onStart={action} onCancel={action} onClear={action} />
      <Clickable key="clickable" padding={{ horizontal: 12, top: 8, bottom: 8 }} gap={6}
        style={{ gap: 10, width: 240, alignItems: "center" }} onClick={action}>
        <Row gap={4}><Text>Grouped box children</Text></Row>
      </Clickable>
      <Column gap={4} padding={{ horizontal: 8, vertical: 4 }}>
        <Text>Column</Text>
      </Column>
    </View>
  );
}
