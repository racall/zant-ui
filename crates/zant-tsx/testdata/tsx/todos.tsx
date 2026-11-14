import { useState, View, Row, Text, Input, Button, Checkbox } from "zant";

interface Todo { id: number; title: string; done: boolean; note?: string }
type Filter = "all" | "active" | "done";

export default function Todos() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [name, setName] = useState("");
  const [nextId, setNextId] = useState(1);
  const [filter, setFilter] = useState<Filter>("all");
  const visible = todos.filter(t => filter === "all" || (filter === "done" ? t.done : !t.done));
  const remaining = todos.filter(t => !t.done).length;
  function add() {
    if (name.length > 0) {
      setTodos(current => [...current, { id: nextId, title: name, done: false }]);
      setNextId(nextId + 1);
      setName("");
    }
  }
  function clear() { setTodos(current => current.filter(t => !t.done)); }
  return <View padding={16} gap={8}>
    <Text key="count">{remaining} remaining ({filter})</Text>
    <Input key="draft" value={name} onChange={setName} onSubmit={add} placeholder="Add a todo" />
    <Row gap={8}>
      <Button key="all" onClick={() => setFilter("all")}>All</Button>
      <Button key="active" onClick={() => setFilter("active")}>Active</Button>
      <Button key="done" onClick={() => setFilter("done")}>Done</Button>
      <Button key="clear" onClick={clear}>Clear completed</Button>
    </Row>
    {visible.map((todo, i) => <Row key={todo.id} gap={8}>
      <Checkbox key={`toggle-${todo.id}`} checked={todo.done} onChange={() =>
        setTodos(current => current.map(t => t.id === todo.id ? { ...t, done: !t.done } : t))}>
        {todo.title}
      </Checkbox>
      <Text>{todo.note ?? "No note"} #{i}</Text>
      <Button key={`remove-${todo.id}`} onClick={() =>
        setTodos(current => current.filter(t => t.id !== todo.id))}>Remove</Button>
    </Row>)}
  </View>;
}
