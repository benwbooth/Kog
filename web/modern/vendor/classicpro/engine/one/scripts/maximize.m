#include <lib/std.mi>
#define DISPATCH
#include ../../scripts/dispatch_codes.m

#include attribs/init_Autoresize.m

Function updateMax();

Global Group frameGroup, cprosui;
Global Button goBig, goSmall, aotBut;
Global Layer topLayer, aotDoc;
Global boolean doubleClick, docked;

Global Container player;
Global Layout normal;

System.onScriptLoaded() {
	initAttribs_Autoresize();

	player = System.getContainer("main");
	normal = player.getLayout("normal");

	frameGroup = getScriptGroup();
	goBig = frameGroup.findObject("main.goBig");
	goSmall = frameGroup.findObject("main.goSmall");
	topLayer = frameGroup.findObject("doubleclick");
	aotBut = frameGroup.findObject("player.aot");
	aotDoc = frameGroup.findObject("player.aot.docked");

	cprosui = normal.findObject("centro.mainframe").getParent();
	doubleClick=false;

	/*double newscalevalue = normal.getScale();
	normal.setXmlParam("maximum_w", integerToString(getViewPortWidthfromGuiObject(normal)/newscalevalue));
	normal.setXmlParam("maximum_h", integerToString(getViewPortHeightfromGuiObject(normal)/newscalevalue));
	normal.setXmlParam("maximum_w", integerToString(System.getMonitorWidth()));
	normal.setXmlParam("maximum_h", integerToString(System.getMonitorHeight()));*/
	//updateMax();
	
	if(getPublicInt("cPro.maximized", 0)==0){
		goBig.show();
		goSmall.hide();
		frameGroup.setXmlParam("lockminmax", "0");
		normal.setXmlParam("move", "1");
	}
	else{
		goBig.hide();
		goSmall.show();
		frameGroup.setXmlParam("lockminmax", "1");
		normal.setXmlParam("move", "0");
	}
}


cprosui.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject source){
	if (strlower(action) == "toggle_fs"){
		if(goBig.isVisible()) goBig.leftClick();
		else goSmall.leftClick();
	}
}

normal.onScale (Double newscalevalue){
	if (normal != player.getCurLayout()) return;
	normal.setXmlParam("maximum_w", integerToString(getViewPortWidthfromGuiObject(normal)/newscalevalue));
	normal.setXmlParam("maximum_h", integerToString(getViewPortHeightfromGuiObject(normal)/newscalevalue));
	
	if(getPublicInt("cPro.maximized", 0)==1){
		normal.resize(getViewPortLeftfromGuiObject(normal), getViewPortTopfromGuiObject(normal), getViewPortWidthfromGuiObject(normal)/newscalevalue, getViewPortHeightfromGuiObject(normal)/newscalevalue);
	}
	updateMax();
}

goBig.onLeftClick(){
	if(docked) return;

	setPublicInt("cPro.playerX", normal.getLeft());
	setPublicInt("cPro.playerY", normal.getTop());
	setPublicInt("cPro.playerW", normal.getWidth());
	setPublicInt("cPro.playerH", normal.getHeight());
	goBig.hide();
	goSmall.show();
	
	normal.setXmlParam("move", "0");
	
	//multi monitor update ;) 
	/*normal.setXmlParam("maximum_w", integerToString(System.getMonitorWidth()));
	normal.setXmlParam("maximum_h", integerToString(System.getMonitorHeight()));*/
	updateMax();

	
	double newscalevalue = normal.getScale();
	normal.resize(getViewPortLeftfromGuiObject(normal), getViewPortTopfromGuiObject(normal), getViewPortWidthfromGuiObject(normal)/newscalevalue, getViewPortHeightfromGuiObject(normal)/newscalevalue);

	setPublicInt("cPro.maximized", 1);
	frameGroup.setXmlParam("lockminmax", "1");
}

goSmall.onLeftClick(){
	if(docked) return;

	goBig.show();
	goSmall.hide();
	frameGroup.resize (getPublicInt("cPro.playerX", 50), getPublicInt("cPro.playerY", 50), getPublicInt("cPro.playerW", 50), getPublicInt("cPro.playerH", 50));
	setPublicInt("cPro.maximized", 0);
	frameGroup.setXmlParam("lockminmax", "0");
	normal.setXmlParam("move", "1");
}

topLayer.onLeftButtonUp(int x, int y){
	if(doubleClick){
		if(titlebar_dblclk_max_attib.getData() == "1"){
			if(goBig.isVisible()){
				goBig.leftClick();
			}
			else{
				goSmall.leftClick();
			}
		}
		else{
			player.switchToLayout("shade");
		}
	}
	doubleClick=false;
}

topLayer.onLeftButtonDblClk(int x, int y){
	doubleClick=true;
}

normal.onDock(int side){
	docked=true;
	aotBut.hide();
	aotDoc.show();
}

normal.onUndock(){
	docked=false;
	aotDoc.hide();
	aotBut.show();
}

updateMax(){
	double newscalevalue = normal.getScale();
	normal.setXmlParam("maximum_w", integerToString(getViewPortWidthfromGuiObject(normal)/newscalevalue));
	normal.setXmlParam("maximum_h", integerToString(getViewPortHeightfromGuiObject(normal)/newscalevalue));
}