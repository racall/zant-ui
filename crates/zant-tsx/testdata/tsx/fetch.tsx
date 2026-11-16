// fetch and promise reactions: typed JSON at the boundary, an effect that
// aborts on cleanup, a POST from a handler, a child's request.
import { useState, useEffect, View, Text, Button } from "zant";

interface Todo {
  id: number;
  title: string;
  completed: boolean;
}

function Detail({ id }: { id: number }) {
  const [title, setTitle] = useState("loading");
  useEffect(() => {
    const controller = new AbortController();
    fetch(`https://jsonplaceholder.typicode.com/todos/${id}`, { signal: controller.signal })
      .then((res) => {
        if (!res.ok) throw new Error("HTTP " + res.status);
        return res.json();
      })
      .then((todo: Todo) => setTitle(todo.title))
      .catch((e) => setTitle(e instanceof Error ? e.message : String(e)));
    return () => controller.abort();
  }, [id]);
  return <Text>{title}</Text>;
}

export default function App() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState("");
  const [title, setTitle] = useState("new");

  useEffect(() => {
    fetch("https://jsonplaceholder.typicode.com/todos?_limit=5", { signal: AbortSignal.timeout(8000) })
      .then((r) => r.json() as Promise<Todo[]>)
      .then(setTodos)
      .catch((e) => setStatus(String(e)))
      .finally(() => setLoading(false));
  }, []);

  function save() {
    fetch("https://jsonplaceholder.typicode.com/todos", {
      method: "POST",
      headers: { "Content-Type": "application/json; charset=UTF-8" },
      body: JSON.stringify({ title: title, completed: false }),
    })
      .then((r) => {
        setStatus(r.ok ? "saved " + r.status : "HTTP " + r.status);
        return r.text();
      })
      .then((text) => console.log(text.length))
      .catch((e) => setStatus("error " + String(e)));
  }

  return (
    <View padding={16} gap={8}>
      {loading ? <Text>Loading</Text> : null}
      {todos.map((todo) => <Text key={todo.id}>{todo.title}</Text>)}
      <Button onClick={save}>Save</Button>
      <Button onClick={() => setTitle(title + "!")}>Rename</Button>
      <Text>{status}</Text>
      <Detail id={1} />
    </View>
  );
}
