// async / await lowered to promise reactions: try/catch/finally, a typed
// Promise<T> helper, a local async function in an effect, locals assigned
// between awaits, an async arrow handler.
import { useState, useEffect, View, Text, Button } from "zant";

interface Todo { id: number; title: string; completed: boolean }
interface Echo { json: { title: string; done: boolean } }

function Detail({ id }: { id: number }) {
  const [title, setTitle] = useState("loading");
  useEffect(() => {
    async function load() {
      const res = await fetch("https://jsonplaceholder.typicode.com/todos/" + id);
      const todo: Todo = await res.json();
      console.log("[event] detail " + todo.id);
      setTitle(todo.title);
    }
    load();
  }, [id]);
  return <Text>{title}</Text>;
}

export default function App() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState("");
  const [steps, setSteps] = useState("");

  async function fetchTodos(limit: number): Promise<Todo[]> {
    const res = await fetch("https://jsonplaceholder.typicode.com/todos?_limit=" + limit);
    if (!res.ok) throw new Error("HTTP " + res.status);
    return res.json();
  }

  async function load() {
    setLoading(true);
    try {
      const list = await fetchTodos(4);
      setTodos(list);
      console.log("[event] loaded " + list.length);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
      console.log("[event] finally");
    }
  }

  useEffect(() => {
    load();
  }, []);

  const save = async () => {
    const res = await fetch("https://httpbin.org/post", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title: "zant", done: true }),
    });
    const echo: Echo = await res.json();
    console.log("[event] echoed " + echo.json.title + " " + echo.json.done);
    setStatus("saved " + res.status);
  };

  async function failing() {
    try {
      await fetch("https://httpbin.org/status/404").then((r) => {
        if (!r.ok) throw new Error("status " + r.status);
      });
      setStatus("unexpected");
    } catch (e) {
      console.log("[event] caught " + (e instanceof Error ? e.message : "?"));
      setStatus("caught");
    }
  }

  async function order() {
    let log = "a";
    const first = await Promise.resolve(1);
    log += first;
    const second = await Promise.resolve(log.length);
    log += second;
    console.log("[event] order " + log);
    setSteps(log);
  }

  return (
    <View padding={16} gap={8}>
      {loading ? <Text>Loading</Text> : null}
      <Text>{error}</Text>
      {todos.map((t) => <Text key={t.id}>{t.title}</Text>)}
      <Button key="save" onClick={save}>Save</Button>
      <Button key="fail" onClick={failing}>Fail</Button>
      <Button key="order" onClick={order}>Order</Button>
      <Text>{status} {steps}</Text>
      <Detail id={2} />
    </View>
  );
}
