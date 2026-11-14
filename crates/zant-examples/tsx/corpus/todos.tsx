// A todo app in TSX, written the React way (immutable state updates).
// The TSX frame benchmark (`zant-tsx-frame-bench`) compares it with the same
// app written by hand in Rust. The runnable copy is `../tsx-todos`.
import { useState, View, Row, Heading, Input, Button, Segmented, Checkbox, IconButton, Text } from "zant";
import type { u32, usize } from "zant";

interface Todo {
  id: u32;
  title: string;
  done: boolean;
}

type Filter = "all" | "active" | "done";

export default function Todos() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [text, setText] = useState("");
  const [nextId, setNextId] = useState<u32>(1);
  const [filter, setFilter] = useState<Filter>("all");

  const left = todos.filter((t) => !t.done).length;
  const visible = todos.filter((t) => (filter === "all" ? true : filter === "done" ? t.done : !t.done));

  function add() {
    if (text.length === 0) return;
    setTodos([...todos, { id: nextId, title: text, done: false }]);
    setNextId(nextId + 1);
    setText("");
  }

  function clearDone() {
    setTodos(todos.filter((t) => !t.done));
  }

  function pickFilter(index: usize) {
    setFilter(index === 0 ? "all" : index === 1 ? "active" : "done");
  }

  return (
    <View padding={24} gap={12}>
      <Heading level={3}>Todos</Heading>
      <Row gap={8}>
        <View flex={1}>
          <Input key="new" value={text} placeholder="What needs to be done?" onChange={setText} onSubmit={add} />
        </View>
        <Button key="add" variant="primary" onClick={add}>
          Add
        </Button>
      </Row>
      <Segmented
        key="filter"
        options={[{ label: "All" }, { label: "Active" }, { label: "Done" }]}
        selected={filter === "all" ? 0 : filter === "active" ? 1 : 2}
        onChange={pickFilter}
      />
      {visible.length === 0 ? (
        <Text key="empty">{todos.length === 0 ? "Nothing to do yet." : "Nothing here."}</Text>
      ) : (
        <View key="rows" gap={4}>
          {visible.map((todo) => (
            <Row key={`todo-${todo.id}`} gap={8} alignItems="center">
              <Checkbox
                key={`check-${todo.id}`}
                checked={todo.done}
                onChange={() => setTodos(todos.map((t) => (t.id === todo.id ? { ...t, done: !t.done } : t)))}
              >
                {todo.title}
              </Checkbox>
              <Row flex={1} />
              <IconButton key={`remove-${todo.id}`} icon="trash" onClick={() => setTodos(todos.filter((t) => t.id !== todo.id))} />
            </Row>
          ))}
        </View>
      )}
      <Row gap={12} alignItems="center">
        <Text key="left">
          {left} {left === 1 ? "item" : "items"} left
        </Text>
        <Button key="clear" onClick={clearDone}>
          Clear completed
        </Button>
      </Row>
    </View>
  );
}
