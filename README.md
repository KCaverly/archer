<br>
<br>
<p align="center">
  <img src="logo.png" width="600"/>
</p>
<br>
<br>

A terminal based interactive interface for working with multi-turn LLMs.
Built on [Replicate](https://replicate.com/) and [Ratatui](https://ratatui.rs/).
<br>

![screenshot](screenshot.png)

Interact with different models within the same conversation. For example, you can ask Mistral to write a haiku, before asking CodeLlama to write a small html page, showing the haiku to the user.

### Getting Started

To interact with models you will need to set your [Replicate API Key](https://replicate.com/account/api-tokens) as the 'REPLICATE_API_KEY' environment variable.

### Platform Specific Functionality

LLMit has only been tested on Linux systems. While all functionality is intended to work cross-platform, there may be concerns working with the clipboard on different platforms. If there are any concerns, please file an Issue.

### Roadmap

In the near term, I am looking to polish the existing experience. Namely, expanding the status notification system to illustrate cold boot progress, better error handling, along with better formatting for code.
Longer term, I would like to integrate a few magic commands, to serve as a full features terminal assistant with directory RAG based functionality.
