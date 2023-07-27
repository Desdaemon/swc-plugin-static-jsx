module.exports = {
  transform: {    
    "^.+\\.(t|j)sx?$": ["@swc/jest", {
      jsc: {
        target: 'esnext',
        parser: {
          syntax: 'typescript',
          tsx: true
        },
        experimental: {
          plugins: [['swc-plugin-static-jsx', {
            template: "html"
          }]]
        }
      }
    }]
  }  
}
