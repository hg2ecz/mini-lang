" Mini language Vim syntax (4.4+).  No runtime/compiler dependency.
if exists('b:current_syntax')
  finish
endif
syn case match
syn keyword miniKeyword fn let return if else while struct program unsafe asm drop pub as
syn keyword miniBool true false
syn keyword miniType bool usize i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 Self
syn match miniDirective /^\s*#\s*\%(if\|elseif\|else\|endif\|include\|error\)\>.*/ contains=miniString,miniComment
syn region miniString start=/"/ skip=/\\./ end=/"/ keepend
syn match miniNumber /\<\d\+\>/
syn match miniComment /\/\/.*$/ contains=miniTodo
syn keyword miniTodo TODO FIXME XXX contained
syn match miniOperator /\%(->\|==\|!=\|<=\|>=\|[+*\/=<>-]\)/
syn match miniFunction /\<\h\w*\ze\s*(/ containedin=ALLBUT,miniComment,miniString,miniDirective
syn match miniAsmRegister /\<\%(rax\|rdi\|rsi\|rdx\|rcx\|r11\|r0\|r1\|r2\|r3\|x0\|x1\|x2\|x8\)\>/ containedin=ALLBUT,miniComment,miniString,miniDirective
hi def link miniKeyword Statement
hi def link miniBool Boolean
hi def link miniType Type
hi def link miniDirective PreProc
hi def link miniString String
hi def link miniNumber Number
hi def link miniComment Comment
hi def link miniTodo Todo
hi def link miniOperator Operator
hi def link miniFunction Function
hi def link miniAsmRegister Identifier
let b:current_syntax = 'mini'
