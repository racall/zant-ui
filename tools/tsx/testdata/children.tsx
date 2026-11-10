import { useState, View, Row, Text, Button, Checkbox, Card } from "zant";
import type { u32 } from "zant";

interface Todo {
  id: u32;
  title: string;
  done: boolean;
}

interface RowProps {
  todo: Todo;
  onToggle: (id: u32) => void;
  onRemove: (id: u32) => void;
  onRename?: (id: u32, title: string) => void;
}

// destructured props, callbacks called from the child's handlers
function TodoRow({ todo, onToggle, onRemove }: RowProps) {
  return (
    <Row gap={8} alignItems="center">
      <Checkbox key={`check-${todo.id}`} checked={todo.done} onChange={() => onToggle(todo.id)}>
        {todo.title}
      </Checkbox>
      <Button key={`remove-${todo.id}`} onClick={() => onRemove(todo.id)}>
        Remove
      </Button>
    </Row>
  );
}

interface PanelProps {
  title: string;
  count: u32;
  children: React.ReactNode[];
  onClear: () => void;
}

// a props object, children and a callback forwarded to a ui event
function Panel(props: PanelProps) {
  return (
    <Card title={props.title}>
      <Text>{props.count} items</Text>
      {props.children}
      <Button key="clear" onClick={props.onClear}>
        Clear
      </Button>
    </Card>
  );
}

export default function App() {
  const [todos, setTodos] = useState<Todo[]>([{ id: 1, title: "Write tests", done: false }]);
  const [last, setLast] = useState("");

  function toggle(id: u32) {
    setTodos(todos.map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  }

  return (
    <View padding={16} gap={8}>
      <Panel title="Todos" count={todos.length as u32} onClear={() => setTodos([])}>
        {todos.map((t) => (
          <TodoRow
            key={t.id}
            todo={t}
            onToggle={toggle}
            onRemove={(id) => setTodos(todos.filter((x) => x.id !== id))}
          />
        ))}
        {last.length > 0 && <Text>Last: {last}</Text>}
      </Panel>
    </View>
  );
}
