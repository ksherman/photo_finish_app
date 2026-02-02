defmodule PhotoFinish.Events.RosterParser do
  @moduledoc """
  Parses roster files into competitor data.
  """

  def parse_txt(content) when is_binary(content) do
    lines =
      content
      |> String.split("\n")
      |> Enum.map(&String.trim/1)
      |> Enum.reject(&(&1 == ""))

    results = Enum.map(lines, &parse_line/1)
    errors = Enum.filter(results, &match?({:error, _}, &1))

    if Enum.empty?(errors) do
      {:ok, Enum.map(results, fn {:ok, c} -> c end)}
    else
      {:error, "Invalid lines found"}
    end
  end

  defp parse_line(line) do
    case String.split(line, " ", parts: 2) do
      [number, name] when byte_size(number) > 0 and byte_size(name) > 0 ->
        if numeric?(number) do
          {first_name, last_name} = split_name(name)
          {:ok, %{competitor_number: number, first_name: first_name, last_name: last_name}}
        else
          {:error, "Invalid line: #{line}"}
        end

      _ ->
        {:error, "Invalid line: #{line}"}
    end
  end

  defp numeric?(string) do
    case Integer.parse(string) do
      {_, ""} -> true
      _ -> false
    end
  end

  defp split_name(name) do
    case String.split(name, " ", parts: 2) do
      [first, last] -> {first, last}
      [first] -> {first, nil}
    end
  end
end
