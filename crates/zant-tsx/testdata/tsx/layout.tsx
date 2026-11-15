import { View, Column, Row, Text, useState } from "zant";

export default function Layout(): JSX.Element {
  const [wide, setWide] = useState(true);
  const overridden = 99;
  const header = <Text key="header">Layout</Text>;
  return (
    <View padding={12} paddingHorizontal={20} paddingTop={8}
      marginVertical={4} marginLeft={6} width="100%" height="75%"
      minWidth={200} maxWidth={1000} minHeight={100} maxHeight={800}
      direction="column" gap={8} rowGap={4} columnGap={6}
      justifyContent={wide ? "space-between" : "flex-start"}
      alignItems="stretch" alignContent="space-around" flexWrap="wrap"
      background="#abc" borderColor="#123456" borderWidth={1}
      hoverBackground="#12345678" hoverBorderColor={0xff1677ff}
      shadowColor="#0008ff" shadowBlur={8} shadowOffsetY={2}
      style={{ radius: overridden, borderRadius: 12, gap: 10 }}>
      {header}
      <>
        <Row flex={1} grow={2} flexShrink={0} basis={10} alignSelf="center"
          padding={6} margin={4} order={1} overflow="hidden" layoutTransition>
          <Text>Row</Text>
        </Row>
        <Column position="relative" overflow="scroll" height={100} width={200}
          justify="end" wrap="nowrap" backgroundColor={wide ? "#fff" : "#001122cc"}>
          <View position="absolute" top={0} right={2} bottom={4} left={6} overflow="visible" />
        </Column>
      </>
      {<><Text>Fragment expression</Text><View paddingBottom={5} /></>}
      {wide && <View key="panel" motion="slideUp" motionOrigin="top"><Text>Panel</Text></View>}
      <Column key="fade" motion="fade"><Text>Fades</Text></Column>
    </View>
  );
}
