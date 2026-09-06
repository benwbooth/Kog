#include <lib/std.mi>

Global Group frameGroup, content;//, titlebar;
Global String x, y, w, h, rx, ry, rw, rh;
Global Layer mouselayer;
Global Button Sysmenu;
Global Text t_title;
Global Group g_titlebar1, g_titlebar2;
Global GuiObject gui_titlebar1, gui_titlebar2;
Global int i_temp;
Global boolean updateOnce, haveTitlebar;

Function setNewGroup(String groupid);

System.onScriptLoaded() {
  frameGroup = getScriptGroup();
  String param = getParam();
  x = getToken(param, ",", 0);
  //y = getToken(param, ",", 1);
  w = getToken(param, ",", 2);
  //h = getToken(param, ",", 3);
  rx = getToken(param, ",", 4);
  ry = getToken(param, ",", 5);
  rw = getToken(param, ",", 6);
  rh = getToken(param, ",", 7);
  sysmenu = frameGroup.findObject("sysmenu");
  t_title = frameGroup.findObject("window.titlebar.title");
  g_titlebar1 = frameGroup.findObject("wasabi.frame.layout.titlebar.1");
  g_titlebar2 = frameGroup.findObject("wasabi.frame.layout.titlebar.2");
  gui_titlebar1 = g_titlebar1.getObject("cpro2.window.titlebar.1");
  gui_titlebar2 = g_titlebar2.getObject("cpro2.window.titlebar.2");
  
	
  	Map myMap = new Map;
	myMap.loadMap("cpro2.genframe.2");
	i_temp = myMap.getHeight();

	myMap.loadMap("cpro2.genframe.titlebar.3");
	if(myMap.getWidth()==10) haveTitlebar = true;
	delete myMap;

	y=integerToString(i_temp);
	h= integerToString(-i_temp-8);

	t_title.setXMLParam("h", y);
	t_title.setXMLParam("fontsize", integerToString(i_temp*0.75));
	
	if(haveTitlebar){
		g_titlebar1.show();
		g_titlebar2.show();
		t_title.setXmlParam("align", "center");
	}

}

System.onSetXuiParam(String param, String value) {
  if (param == "content") {
    setNewGroup(value);
   /* titlebar = frameGroup.findObject("wasabi.titlebar");
    mouselayer = titlebar.findObject("mousetrap");*/
  }
  /*if (param == "padtitleright" || param == "padtitleleft") {
    if (titlebar != NULL) titlebar.setXmlParam(param, value); 
  }
  if (param == "shade") {
    if (mouselayer != NULL) mouselayer.setXmlParam("dblclickaction", "switch;"+value);
    else messagebox("Cannot set shade parameter for StandardFrame object, no mousetrap found", "Skin Error", 0, "");
  }*/
}

// backward compatibility for prerelease notify trick
frameGroup.onNotify(String cmd, String param, int a, int b) {
  String _command = getToken(cmd, ",", 0);
  String _param = getToken(cmd, ",", 1);
  if (_command == "content" || _command == "padtitleright" || _command == "padtitleleft" || _command == "shade") {
    onSetXuiParam(_command, _param);
  }
}

t_title.onTextChanged(String newTxt){
	if(haveTitlebar){
		if(t_title.getXmlParam("text") != newTxt){
			updateOnce = true;
			t_title.setText(newtxt);
		}
		
		i_temp = t_title.getTextWidth();
		
		gui_titlebar1.setXmlParam("w", integerToString(-8-i_temp/2));
		gui_titlebar2.setXmlParam("x", integerToString(i_temp/2+2));
		gui_titlebar2.setXmlParam("w", integerToString(-i_temp/2-2-25-8-2));
	}

}
frameGroup.onSetVisible(boolean onOff){
	if(onOff) t_title.onTextChanged(t_title.getText());
}

setNewGroup(String groupid) {
  content = newGroup(groupid);
  if (content == NULL) {
    messagebox("group \"" + groupid + "\" not found", "ButtonGroup", 0, "");
    return;
  }
  content.setXmlParam("x", x);
  content.setXmlParam("y", y);
  content.setXmlParam("w", w);
  content.setXmlParam("h", h);
  content.setXmlParam("relatx", rx);
  content.setXmlParam("relaty", ry);
  content.setXmlParam("relatw", rw);
  content.setXmlParam("relath", rh);
  content.init(frameGroup);
}
/*
Sysmenu.onLeftClick() {
  LayoutStatus _status = frameGroup.findObject("sysmenu.status");
  _status.callme("{system}");
}*/
